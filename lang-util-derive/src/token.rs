use std::collections::HashMap;

use darling::{FromDeriveInput, FromField, FromMeta, FromVariant};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, DeriveInput, Token};

#[derive(FromField)]
struct TokenVariantField {
    ident: Option<syn::Ident>,
}

struct TokenDisplay {
    format: String,
    args: Vec<String>,
}

impl FromMeta for TokenDisplay {
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self {
            format: value.to_owned(),
            args: vec![],
        })
    }

    fn from_list(items: &[syn::NestedMeta]) -> darling::Result<Self> {
        Ok(Self {
            format: String::from_nested_meta(
                items
                    .first()
                    .ok_or_else(|| darling::Error::custom("missing format string"))?,
            )?,
            args: items
                .iter()
                .skip(1)
                .map(String::from_nested_meta)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

struct TokenAttr {
    token: String,
}

impl FromMeta for TokenAttr {
    fn from_list(items: &[syn::NestedMeta]) -> darling::Result<Self> {
        Ok(Self {
            token: String::from_nested_meta(
                items
                    .first()
                    .ok_or_else(|| darling::Error::custom("missing token literal"))?,
            )?,
        })
    }
}

enum AsParser {
    Path(syn::Path),
    RawString(String),
}

impl FromMeta for AsParser {
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self::RawString(value.to_owned()))
    }

    fn from_nested_meta(item: &syn::NestedMeta) -> darling::Result<Self> {
        match item {
            syn::NestedMeta::Meta(syn::Meta::Path(p)) => Ok(Self::Path(p.to_owned())),
            _ => Err(darling::Error::unsupported_format("meta")),
        }
    }

    fn from_list(items: &[syn::NestedMeta]) -> darling::Result<Self> {
        if let Some(item) = items.first() {
            return Self::from_nested_meta(item);
        }

        Err(darling::Error::unsupported_format("list"))
    }
}

#[derive(FromVariant)]
#[darling(attributes(lang_util), forward_attrs(token))]
struct TokenVariant {
    ident: syn::Ident,
    fields: darling::ast::Fields<TokenVariantField>,
    attrs: Vec<syn::Attribute>,
    #[darling(default)]
    display: Option<TokenDisplay>,
    #[darling(default, rename = "as")]
    as_parser: Option<AsParser>,
}

struct Token<'s> {
    variant: &'s TokenVariant,
    token: TokenAttrTy<'s>,
    as_parser: Result<String, AsParserError>,
}

impl<'s> Token<'s> {
    fn as_parser_token_arm(&self, base_ident: &syn::Ident) -> TokenStream {
        let variant_name = &self.variant.ident;
        let variant_header = match &self.variant.fields.style {
            darling::ast::Style::Tuple => {
                let fields = self
                    .variant
                    .fields
                    .fields
                    .iter()
                    .map(|_| Token![_](Span::call_site()));

                quote! { #variant_name ( #(#fields),* ) }
            }
            darling::ast::Style::Struct => {
                quote! { .. }
            }
            darling::ast::Style::Unit => quote! { #variant_name },
        };

        let body = match &self.as_parser {
            Ok(value) => quote_spanned! { self.variant.ident.span() => #value },
            Err(error) => {
                let error = error.to_string();
                quote_spanned! {
                    self.variant.ident.span() =>
                        compile_error!(#error)
                }
            }
        };

        quote_spanned! {
            self.variant.ident.span() =>
                #base_ident :: #variant_header => #body
        }
    }

    fn display_arm_body(&self, declared_fields: &[syn::Ident]) -> TokenStream {
        match self.variant.fields.style {
            darling::ast::Style::Tuple => {
                if let Some(display) = &self.variant.display {
                    let fmt = &display.format;

                    // TODO: Better than this ugly format string parsing?
                    let args = if display.args.is_empty() {
                        if declared_fields.len() == 1 && display.format.contains("{}") {
                            let f = &declared_fields[0];
                            vec![quote_spanned! { self.variant.ident.span() => #f }]
                        } else {
                            declared_fields
                                .iter()
                                .enumerate()
                                .filter_map(|(i, df)| {
                                    let ph = format!("_{}", i);

                                    if fmt.contains(&ph) {
                                        let ph = format_ident!("{}", ph);
                                        Some(quote_spanned! { self.variant.ident.span() => #ph = #df })
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        }
                    } else {
                        let mut repl = Vec::new();
                        for (i, df) in declared_fields.iter().enumerate() {
                            repl.push((format!("_{}", i), df.to_string()));
                        }

                        display
                            .args
                            .iter()
                            .map(|arg| {
                                let mut arg = arg.clone();
                                for (src, dst) in &repl {
                                    arg = arg.replace(src, dst);
                                }

                                syn::parse_str(&arg).expect("parsing error")
                            })
                            .collect()
                    };

                    return quote_spanned! {
                        self.variant.ident.span() =>
                            write!(f, #fmt, #(#args),*)
                    };
                } else {
                    // No display attribute, check if we can have a sensible default
                    if declared_fields.len() == 1 {
                        let v1 = &declared_fields[0];
                        return quote_spanned! {
                            self.variant.ident.span() =>
                                write!(f, "{}", #v1)
                        };
                    }
                }
            }
            darling::ast::Style::Struct => {
                return quote_spanned! {
                    self.variant.ident.span() =>
                        compile_error!("struct variants are not supported yet by derive(Token)")
                };
            }
            darling::ast::Style::Unit => {
                // Unit struct, is there a token impl?
                if let Some((token, attr)) = &self.token {
                    return match token {
                        Ok(value) => {
                            let value = &value.token;
                            quote_spanned! { self.variant.ident.span() => write!(f, "{}", #value) }
                        }
                        Err(error) => {
                            let s = format!("invalid token attribute: {}", error);
                            quote_spanned! {
                                attr.path.span() =>
                                    compile_error!(#s)
                            }
                        }
                    };
                } else if let Some(display) = &self.variant.display {
                    let value = &display.format;
                    return quote_spanned! { self.variant.ident.span() => write!(f, "{}", #value) };
                }
            }
        }

        quote_spanned! {
            self.variant.ident.span() =>
                compile_error!("missing token or lang_util(display(...)) attributes")
        }
    }

    fn display_arm(&self, base_ident: &syn::Ident) -> TokenStream {
        // Create arm header
        let variant_name = &self.variant.ident;
        let (variant_header, declared_fields) = match &self.variant.fields.style {
            darling::ast::Style::Tuple => {
                let fields: Vec<_> = self
                    .variant
                    .fields
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, _field)| format_ident!("value_{}", i))
                    .collect();

                (
                    quote_spanned! { self.variant.ident.span() => #variant_name ( #(#fields),* ) },
                    fields,
                )
            }
            darling::ast::Style::Struct => {
                let fields: Vec<_> = self
                    .variant
                    .fields
                    .fields
                    .iter()
                    .map(|field| field.ident.as_ref().unwrap().clone())
                    .collect();

                (
                    quote_spanned! { self.variant.ident.span() => #variant_name { #(#fields),* } },
                    fields,
                )
            }
            darling::ast::Style::Unit => (quote! { #variant_name }, vec![]),
        };

        // Create arm body
        let body = self.display_arm_body(&declared_fields);

        quote_spanned! {
            self.variant.ident.span() =>
                #base_ident :: #variant_header => { #body }
        }
    }
}

type TokenAttrTy<'s> = Option<(darling::Result<TokenAttr>, &'s syn::Attribute)>;

fn parse_token_attr(attrs: &[syn::Attribute]) -> TokenAttrTy {
    // Unit struct, is there a token impl?
    if let Some(token) = attrs.iter().find(|attr| {
        attr.path
            .get_ident()
            .map(|ident| ident == "token")
            .unwrap_or(false)
    }) {
        Some((
            token
                .parse_meta()
                .map_err(darling::Error::custom)
                .and_then(|meta| TokenAttr::from_meta(&meta)),
            token,
        ))
    } else {
        None
    }
}

#[derive(Debug)]
enum AsParserError {
    MissingDisplayImpl,
    InvalidAs,
    InvalidTokenAttribute(String),
    NoTokenOrAs,
}

impl std::fmt::Display for AsParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDisplayImpl => write!(f, "in lang_util attribute, `as = display` specified but not display implementation provided"),
            Self::InvalidAs => write!(f, "invalid `as` value, expected display or a literal string"),
            Self::InvalidTokenAttribute(error) => write!(f, "invalid token attribute: {}", error),
            Self::NoTokenOrAs => write!(f, "missing token or lang_util(as = \"...\") attributes"),
        }
    }
}

fn parse_as_parser(variant: &TokenVariant, token: &TokenAttrTy) -> Result<String, AsParserError> {
    if let Some(as_parser) = &variant.as_parser {
        match as_parser {
            AsParser::Path(path) => {
                if path.get_ident().map(|id| id == "display").unwrap_or(false) {
                    if let Some(display) = &variant.display {
                        Ok(format!("\"{}\"", &display.format))
                    } else {
                        Err(AsParserError::MissingDisplayImpl)
                    }
                } else {
                    Err(AsParserError::InvalidAs)
                }
            }
            AsParser::RawString(s) => Ok(s.to_owned()),
        }
    } else if let Some((token, _)) = &token {
        match token {
            Ok(value) => {
                // Tokens would be wrapped with quotes
                Ok(format!("\"{}\"", &value.token))
            }
            Err(error) => Err(AsParserError::InvalidTokenAttribute(error.to_string())),
        }
    } else {
        Err(AsParserError::NoTokenOrAs)
    }
}

impl<'s> From<&'s TokenVariant> for Token<'s> {
    fn from(variant: &'s TokenVariant) -> Self {
        let token = parse_token_attr(&variant.attrs);
        let as_parser = parse_as_parser(&variant, &token);

        Self {
            variant,
            token,
            as_parser,
        }
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(lang_util))]
struct TokenOpts {
    ident: syn::Ident,
    generics: syn::Generics,
    data: darling::ast::Data<TokenVariant, ()>,
}

fn display_impl(
    base_ident: &syn::Ident,
    enum_name: &TokenStream,
    variants: &[Token],
) -> TokenStream {
    let arms = variants.iter().map(|v| v.display_arm(base_ident));

    quote_spanned! {
        base_ident.span() =>
            impl ::std::fmt::Display for #enum_name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    match self {
                        #(#arms),*
                    }
                }
            }
    }
}

fn token_impl(base_ident: &syn::Ident, enum_name: &TokenStream, variants: &[Token]) -> TokenStream {
    let arms = variants.iter().map(|v| v.as_parser_token_arm(base_ident));

    quote_spanned! {
        base_ident.span() =>
            impl ::lang_util::error::Token for #enum_name {
                fn as_parser_token(&self) -> &'static str {
                    match self {
                        #(#arms),*
                    }
                }
            }
    }
}

pub(crate) fn token(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let opts = {
        // Parse the input tokens into a syntax tree
        let input = parse_macro_input!(input as DeriveInput);
        // Find out enum-level options
        TokenOpts::from_derive_input(&input).expect("failed to parse options")
    };

    // Extract enum fields
    let fields = match &opts.data {
        darling::ast::Data::Enum(fields) => fields,
        darling::ast::Data::Struct(_) => {
            return proc_macro::TokenStream::from(quote_spanned! {
                opts.ident.span() =>
                    compile_error!("only enums can be used as tokens with derive(Token)")
            })
        }
    };

    // Create the Token structs
    let fields: Vec<Token> = fields.iter().map(Into::into).collect();

    // All declarations
    let mut decls = Vec::new();

    // Check unicity of the as declarations
    let mut seen: HashMap<_, &Token> = HashMap::new();
    for decl in &fields {
        if let Ok(as_parser) = &decl.as_parser {
            if let Some(previous) = seen.get(as_parser) {
                let s = format!(
                    "`{}` parser token already declared by variant {}",
                    as_parser, previous.variant.ident
                );
                decls.push(quote_spanned! { decl.variant.ident.span() => compile_error!(#s); });
            } else {
                seen.insert(as_parser, decl);
            }
        }
    }

    // Compute enum name
    let base_ident = &opts.ident;
    let enum_name = {
        // Add anonymous lifetimes as needed
        let lifetimes: Vec<_> = opts.generics.lifetimes().map(|_| quote! { '_ }).collect();

        if lifetimes.is_empty() {
            quote! { #base_ident }
        } else {
            quote! { #base_ident<#(#lifetimes),*> }
        }
    };

    decls.push(display_impl(&base_ident, &enum_name, &fields));
    decls.push(token_impl(&base_ident, &enum_name, &fields));

    proc_macro::TokenStream::from(quote_spanned! {
        opts.ident.span() =>
            #(#decls)*
    })
}