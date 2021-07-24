use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    convert::TryInto,
    iter::{FromIterator, FusedIterator},
    num::NonZeroU32,
    path::{Path, PathBuf},
    rc::Rc,
};

use smol_str::SmolStr;

use crate::{
    parser::{Parser, SyntaxKind::*},
    Ast, FileId,
};

#[macro_use]
pub mod exts;

mod event;
pub use event::*;

mod fs;
pub use fs::*;

pub mod nodes;
use nodes::{
    Define, DefineObject, DirectiveResult, Error, Extension, ExtensionName, IfDef, IfNDef, Undef,
    Version, GL_ARB_SHADING_LANGUAGE_INCLUDE, GL_GOOGLE_INCLUDE_DIRECTIVE,
};

/// Operating mode for #include directives
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeMode {
    /// No #include directives are allowed
    None,
    /// GL_ARB_shading_language_include runtime includes
    ArbInclude,
    /// GL_GOOGLE_include_directive compile-time includes
    GoogleInclude,
}

impl Default for IncludeMode {
    fn default() -> Self {
        Self::None
    }
}

/// Current state of the preprocessor
#[derive(Debug, Clone)]
pub struct ProcessorState {
    extension_stack: Vec<Extension>,
    include_mode: IncludeMode,
    // use Rc to make cloning the whole struct cheaper
    definitions: HashMap<SmolStr, Definition>,
    version: Version,
}

#[derive(Debug, Clone)]
enum Definition {
    Regular(Rc<Define>, FileId),
    Line,
    File,
    Version,
}

impl Definition {
    pub fn protected(&self) -> bool {
        match self {
            Definition::Regular(d, _) => d.protected(),
            Definition::Line => true,
            Definition::File => true,
            Definition::Version => true,
        }
    }
}

impl Default for ProcessorState {
    fn default() -> Self {
        Self {
            // Spec 3.3, "The initial state of the compiler is as if the directive
            // `#extension all : disable` was issued
            extension_stack: vec![Extension::disable(ExtensionName::All)],
            // No #include extensions enabled
            include_mode: IncludeMode::None,
            // Spec 3.3, "There is a built-in macro definition for each profile the implementation
            // supports. All implementations provide the following macro:
            // `#define GL_core_profile 1`
            definitions: HashMap::from_iter([
                (
                    "GL_core_profile".into(),
                    Definition::Regular(
                        Rc::new(Define::object(
                            "GL_core_profile".into(),
                            DefineObject::from_str("1").unwrap(),
                            true,
                        )),
                        FileId::default(),
                    ),
                ),
                ("__LINE__".into(), Definition::Line),
                ("__FILE__".into(), Definition::File),
                ("__VERSION__".into(), Definition::Version),
            ]),
            version: Version::default(),
        }
    }
}

/// Preprocessor
#[derive(Debug)]
pub struct Processor<F: FileSystem> {
    /// Cache of parsed files (preprocessor token sequences)
    file_cache: HashMap<FileId, Ast>,
    /// Mapping from canonical paths to FileIds
    file_ids: HashMap<PathBuf, FileId>,
    /// Mapping from #include/input paths to canonical paths
    canonical_paths: HashMap<PathBuf, PathBuf>,
    /// Current state of the preprocessor
    current_state: ProcessorState,
    /// Filesystem abstraction
    fs: F,
}

impl<F: FileSystem + Default> Processor<F> {
    pub fn new(initial_state: ProcessorState) -> Self {
        Self {
            current_state: initial_state,
            ..Default::default()
        }
    }
}

impl<F: FileSystem> Processor<F> {
    pub fn new_with_fs(initial_state: ProcessorState, fs: F) -> Self {
        Self {
            file_cache: HashMap::with_capacity(1),
            file_ids: HashMap::with_capacity(1),
            canonical_paths: HashMap::with_capacity(1),
            current_state: initial_state,
            fs,
        }
    }

    pub fn process(&mut self, entry: &Path) -> ProcessorEvents<F> {
        ProcessorEvents {
            processor: Some(self),
            file_stack: vec![entry.to_owned()],
            event_buf: Default::default(),
        }
    }

    fn parse(&mut self, path: &Path) -> Result<(FileId, &Ast), F::Error> {
        // Find the canonical path. Not using the entry API because cloning a path is expensive.
        let canonical_path = if let Some(canonical_path) = self.canonical_paths.get(path) {
            canonical_path
        } else {
            let canonical_path = self.fs.canonicalize(&path)?;
            // Using entry allows inserting and returning a reference
            self.canonical_paths
                .entry(path.to_owned())
                .or_insert(canonical_path)
        };

        // Allocate a file id. Not using the entry API because cloning a path is expensive.
        let file_id = if let Some(file_id) = self.file_ids.get(canonical_path.as_path()) {
            *file_id
        } else {
            // SAFETY: n + 1 > 0
            let file_id =
                FileId::new(unsafe { NonZeroU32::new_unchecked(self.file_ids.len() as u32 + 1) });
            self.file_ids.insert(canonical_path.to_owned(), file_id);
            file_id
        };

        match self.file_cache.entry(file_id) {
            Entry::Occupied(entry) => Ok((file_id, entry.into_mut())),
            Entry::Vacant(entry) => {
                // Read the file
                let input = self.fs.read(&canonical_path)?;
                // Parse it
                let ast = Parser::new(&input).parse();
                Ok((file_id, entry.insert(ast)))
            }
        }
    }

    fn expand(&mut self, ast: Ast, current_file: FileId) -> Vec<Event<F::Error>> {
        let (root, mut errors) = ast.into_inner();

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum IfState {
            /// No #if group of this level was included
            None,
            /// The current #if group of this level is included
            Active { else_seen: bool },
            /// One past #if group of this level was included, but not the current one
            One { else_seen: bool },
        }

        // TODO: Smarter capacity calculation
        let mut result = Vec::with_capacity(1024);
        let mut mask_stack = Vec::with_capacity(4);
        let mut mask_active = true;

        for node_or_token in root.children_with_tokens() {
            if let Some(first) = errors.first() {
                if node_or_token.text_range().end() >= first.pos().start() {
                    let error = errors.pop().unwrap();

                    // Parse errors in non-included blocks are ignored
                    if mask_active {
                        result.push(Event::error(error));
                    }
                }
            }

            match node_or_token {
                rowan::NodeOrToken::Node(node) => {
                    match node.kind() {
                        PP_EMPTY => {
                            // Discard
                        }
                        PP_VERSION => {
                            if mask_active {
                                // TODO: Check that the version is the first thing in the file?

                                let directive: DirectiveResult<Version> = node.try_into();

                                if let Ok(version) = &directive {
                                    self.current_state.version = **version;
                                }

                                result.push(Event::directive(directive));
                            }
                        }
                        PP_EXTENSION => {
                            if mask_active {
                                let directive: DirectiveResult<Extension> = node.try_into();

                                if let Ok(extension) = &directive {
                                    // Push onto the stack
                                    self.current_state
                                        .extension_stack
                                        .push((**extension).clone());

                                    let target_include_mode =
                                        if extension.name == *GL_ARB_SHADING_LANGUAGE_INCLUDE {
                                            Some(IncludeMode::ArbInclude)
                                        } else if extension.name == *GL_GOOGLE_INCLUDE_DIRECTIVE {
                                            Some(IncludeMode::GoogleInclude)
                                        } else {
                                            None
                                        };

                                    if let Some(target) = target_include_mode {
                                        if extension.behavior.is_active() {
                                            self.current_state.include_mode = target;
                                        } else {
                                            // TODO: Implement current mode as a stack?
                                            self.current_state.include_mode = IncludeMode::None;
                                        }
                                    }
                                }

                                result.push(Event::directive(directive));
                            }
                        }
                        PP_DEFINE => {
                            if mask_active {
                                let directive: DirectiveResult<Define> = node.clone().try_into();

                                let error = if let Ok(define) = &directive {
                                    if define.name().starts_with("GL_") {
                                        Some(
                                            ProcessingErrorKind::ProtectedDefine {
                                                ident: define.name().into(),
                                                is_undef: false,
                                            }
                                            .with_node(node),
                                        )
                                    } else {
                                        let definition = Definition::Regular(
                                            Rc::new((**define).clone()),
                                            current_file,
                                        );

                                        match self
                                            .current_state
                                            .definitions
                                            .entry(define.name().into())
                                        {
                                            Entry::Occupied(mut entry) => {
                                                if entry.get().protected() {
                                                    Some(
                                                        ProcessingErrorKind::ProtectedDefine {
                                                            ident: define.name().into(),
                                                            is_undef: false,
                                                        }
                                                        .with_node(node),
                                                    )
                                                } else {
                                                    // TODO: Check that we are not overwriting an incompatible definition
                                                    *entry.get_mut() = definition;

                                                    None
                                                }
                                            }
                                            Entry::Vacant(entry) => {
                                                entry.insert(definition);
                                                None
                                            }
                                        }
                                    }
                                } else {
                                    None
                                };

                                result.push(Event::directive(directive));

                                if let Some(error) = error {
                                    result.push(Event::error(error));
                                }
                            }
                        }
                        PP_IFDEF => {
                            if mask_active {
                                let directive: DirectiveResult<IfDef> = node.try_into();

                                if let Ok(ifdef) = &directive {
                                    // Update masking state
                                    mask_active =
                                        self.current_state.definitions.contains_key(&ifdef.ident);
                                    mask_stack.push(IfState::Active { else_seen: false });
                                }

                                result.push(Event::directive(directive));
                            } else {
                                // Record the #ifdef in the stack to support nesting
                                mask_stack.push(IfState::None);
                            }
                        }
                        PP_IFNDEF => {
                            if mask_active {
                                let directive: DirectiveResult<IfNDef> = node.try_into();

                                if let Ok(ifdef) = &directive {
                                    // Update masking state
                                    mask_active =
                                        !self.current_state.definitions.contains_key(&ifdef.ident);
                                    mask_stack.push(IfState::Active { else_seen: false });
                                }

                                result.push(Event::directive(directive));
                            } else {
                                // Record the #ifdef in the stack to support nesting
                                mask_stack.push(IfState::None);
                            }
                        }
                        PP_ELSE => {
                            if let Some(top) = mask_stack.pop() {
                                match top {
                                    IfState::None => {
                                        mask_active = mask_stack
                                            .last()
                                            .map(|top| matches!(*top, IfState::Active { .. }))
                                            .unwrap_or(true);

                                        mask_stack.push(IfState::Active { else_seen: true });
                                    }
                                    IfState::Active { else_seen } | IfState::One { else_seen } => {
                                        if else_seen {
                                            // Extra #else
                                            result.push(Event::error(
                                                ProcessingErrorKind::ExtraElse.with_node(node),
                                            ));

                                            continue;
                                        } else {
                                            mask_active = false;
                                            mask_stack.push(IfState::One { else_seen: true });
                                        }
                                    }
                                }

                                result.push(Event::directive(DirectiveKind::Else));
                            } else {
                                // Stray #else
                                result.push(Event::error(
                                    ProcessingErrorKind::ExtraElse.with_node(node),
                                ));
                            }
                        }
                        PP_ENDIF => {
                            if let Some(_) = mask_stack.pop() {
                                mask_active = mask_stack
                                    .last()
                                    .map(|top| matches!(*top, IfState::Active { .. }))
                                    .unwrap_or(true);

                                // TODO: Return syntax node?
                                if mask_active {
                                    result.push(Event::directive(DirectiveKind::EndIf));
                                }
                            } else {
                                // Stray #endif
                                result.push(Event::error(
                                    ProcessingErrorKind::ExtraEndIf.with_node(node),
                                ));
                            }
                        }
                        PP_UNDEF => {
                            if mask_active {
                                let directive: DirectiveResult<Undef> = node.clone().try_into();

                                let protected_ident = if let Ok(ifdef) = &directive {
                                    if ifdef.ident.starts_with("GL_") {
                                        Some(ifdef.ident.clone())
                                    } else {
                                        if let Some(def) =
                                            self.current_state.definitions.get(&ifdef.ident)
                                        {
                                            if def.protected() {
                                                Some(ifdef.ident.clone())
                                            } else {
                                                self.current_state.definitions.remove(&ifdef.ident);
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };

                                result.push(Event::directive(directive));

                                if let Some(ident) = protected_ident {
                                    result.push(Event::error(
                                        ProcessingErrorKind::ProtectedDefine {
                                            ident,
                                            is_undef: true,
                                        }
                                        .with_node(node),
                                    ));
                                }
                            }
                        }
                        PP_ERROR => {
                            if mask_active {
                                let directive: DirectiveResult<Error> = node.clone().try_into();

                                let error = if let Ok(error) = &directive {
                                    Some(Event::error(
                                        ProcessingErrorKind::ErrorDirective {
                                            message: error.message.clone(),
                                        }
                                        .with_node(node),
                                    ))
                                } else {
                                    None
                                };

                                result.push(Event::directive(directive));

                                if let Some(error_event) = error {
                                    result.push(error_event);
                                }
                            }
                        }
                        _ => {
                            // Handle node, this is a preprocessor directive
                            result.push(Event::error(ErrorKind::Unhandled(node)));
                        }
                    }
                }
                rowan::NodeOrToken::Token(token) => {
                    if mask_active {
                        result.push(Event::Token(token));
                    }
                }
            }
        }

        result
    }
}

pub struct ProcessorEvents<'p, F: FileSystem> {
    processor: Option<&'p mut Processor<F>>,
    file_stack: Vec<PathBuf>,
    event_buf: VecDeque<Event<F::Error>>,
}

impl<'p, F: FileSystem> Iterator for ProcessorEvents<'p, F> {
    type Item = Event<F::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(processor) = &mut self.processor {
                // First, check if we have buffered any events
                if let Some(event) = self.event_buf.pop_front() {
                    return Some(event);
                }

                // Then, check how we can generate more events
                if let Some(file) = self.file_stack.pop() {
                    // An unprocessed file
                    match processor.parse(&file) {
                        Ok((file_id, ast)) => {
                            let ast = ast.clone();

                            // We entered a file
                            self.event_buf.push_back(Event::EnterFile {
                                file_id,
                                path: file.to_owned(),
                            });

                            // Add all preprocessor events
                            self.event_buf.extend(processor.expand(ast, file_id));

                            continue;
                        }
                        Err(err) => {
                            // Failed reading the file
                            return Some(Event::error(ErrorKind::Io(err)));
                        }
                    }
                }

                // If we get here, there are no more events we can generate
                self.processor.take();
                return None;
            } else {
                return None;
            }
        }
    }
}

impl<F: FileSystem> FusedIterator for ProcessorEvents<'_, F> {}

impl<F: FileSystem + Default> Default for Processor<F> {
    fn default() -> Self {
        Self {
            file_cache: HashMap::with_capacity(1),
            file_ids: HashMap::with_capacity(1),
            canonical_paths: HashMap::with_capacity(1),
            current_state: Default::default(),
            fs: F::default(),
        }
    }
}
