use boltffi_binding::{CustomConverterPath, CustomConverterPathRoot, CustomTypeConverter};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprPath, parse_str};

use crate::expansion::error::Error;

pub fn tokens(converter: &CustomTypeConverter) -> Result<TokenStream, Error> {
    match converter {
        CustomTypeConverter::Path(path) => path_tokens(path),
        CustomTypeConverter::TraitMethod(converter) => {
            let receiver = receiver_type(converter.receiver())?;
            let method = parse_str::<syn::Ident>(converter.method().as_str()).map_err(|_| {
                Error::SourceSyntaxMismatch("custom converter method is not Rust syntax")
            })?;
            Ok(quote! {
                <#receiver as ::boltffi::CustomFfiConvertible>::#method
            })
        }
        CustomTypeConverter::Expression(expression) => parse_str::<Expr>(expression.source())
            .map(|expression| quote! { #expression })
            .map_err(|_| Error::SourceSyntaxMismatch("custom converter is not Rust syntax")),
        _ => Err(Error::UnsupportedExpansion("unknown custom converter")),
    }
}

fn path_tokens(path: &CustomConverterPath) -> Result<TokenStream, Error> {
    parse_str::<ExprPath>(&source(path)?)
        .map(|path| quote! { #path })
        .map_err(|_| Error::SourceSyntaxMismatch("custom converter path is not Rust syntax"))
}

fn receiver_type(path: &CustomConverterPath) -> Result<syn::Type, Error> {
    parse_str::<syn::Type>(&source(path)?)
        .map_err(|_| Error::SourceSyntaxMismatch("custom converter receiver is not Rust syntax"))
}

fn source(path: &CustomConverterPath) -> Result<String, Error> {
    let prefix = match path.root() {
        CustomConverterPathRoot::Relative => String::new(),
        CustomConverterPathRoot::Crate => "crate::".to_owned(),
        CustomConverterPathRoot::Self_ => "self::".to_owned(),
        CustomConverterPathRoot::Super(levels) => {
            std::iter::repeat_n("super", levels.get())
                .collect::<Vec<_>>()
                .join("::")
                + "::"
        }
        CustomConverterPathRoot::Absolute => "::".to_owned(),
        _ => {
            return Err(Error::UnsupportedExpansion(
                "unknown custom converter path root",
            ));
        }
    };
    let segments = path
        .segments()
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<_>>()
        .join("::");
    Ok(prefix + &segments)
}
