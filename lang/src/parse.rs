use crate::{ast, lexer::Lexer, parser};

pub use crate::lexer::{
    CommentList, Comments, IdentifierContext, LexerPosition, LexicalError, Token, TokenKind,
    TypeNames, TypeTablePolicy,
};

mod parsable;
pub use parsable::Parsable;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ParseOptions {
    pub target_vulkan: bool,
    pub source_id: usize,
    pub allow_rs_ident: bool,
    pub type_names: TypeNames,
    pub comments: Option<Comments>,
}

impl ParseOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_comments() -> Self {
        Self {
            comments: Some(Comments::default()),
            ..Default::default()
        }
    }
}

pub type ParseError<'i> = lalrpop_util::ParseError<LexerPosition, Token<'i>, LexicalError>;
pub type ParseErrorStatic = lalrpop_util::ParseError<LexerPosition, TokenKind, LexicalError>;

pub trait LangParser: Sized {
    fn new() -> Self;
}

pub trait Parse: Sized {
    type Parser: LangParser;

    fn parse(source: &str) -> Result<Self, ParseError> {
        <Self as Parse>::parse_with_options(source, &Default::default())
            .map(|(parsed, _names)| parsed)
    }

    fn parse_with_options<'i>(
        source: &'i str,
        opts: &ParseOptions,
    ) -> Result<(Self, ParseOptions), ParseError<'i>>;

    fn parse_with_parser<'i>(
        source: &'i str,
        opts: &ParseOptions,
        parser: &Self::Parser,
    ) -> Result<(Self, ParseOptions), ParseError<'i>>;
}

macro_rules! impl_parse {
    ($t:ty => $p:ty) => {
        impl LangParser for $p {
            fn new() -> Self {
                <$p>::new()
            }
        }

        impl Parse for $t {
            type Parser = $p;

            fn parse_with_options<'i>(
                source: &'i str,
                opts: &ParseOptions,
            ) -> Result<(Self, ParseOptions), ParseError<'i>> {
                let parser = <$p>::new();
                Self::parse_with_parser(source, opts, &parser)
            }

            fn parse_with_parser<'i>(
                source: &'i str,
                opts: &ParseOptions,
                parser: &$p,
            ) -> Result<(Self, ParseOptions), ParseError<'i>> {
                // Clone the input structure
                let cloned_opts = opts.clone();

                // Clone the type names
                let cloned_opts = ParseOptions {
                    type_names: cloned_opts.type_names.clone_inner(),
                    comments: cloned_opts.comments.map(|c| c.clone_inner()),
                    ..cloned_opts
                };

                // Invoke the parser
                let lexer = Lexer::new(source, cloned_opts.clone());
                parser
                    .parse(source, lexer)
                    .map(|parsed| (parsed, cloned_opts))
            }
        }
    };
}

#[cfg(feature = "parser-expr")]
impl_parse!(ast::Expr            => parser::ExprParser);
#[cfg(feature = "parser-preprocessor")]
impl_parse!(ast::Preprocessor    => parser::PreprocessorParser);
#[cfg(feature = "parser-statement")]
impl_parse!(ast::Statement       => parser::StatementParser);
impl_parse!(ast::TranslationUnit => parser::TranslationUnitParser);
