use boltffi_ast::Visibility;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Path, parse_str};

use crate::expansion::error::Error;

pub struct VisibilityTokens {
    visibility: Visibility,
}

impl VisibilityTokens {
    pub fn new(visibility: &Visibility) -> Self {
        Self {
            visibility: visibility.clone(),
        }
    }

    pub fn into_tokens(self) -> Result<TokenStream, Error> {
        match &self.visibility {
            Visibility::Private => Ok(TokenStream::new()),
            Visibility::Public => Ok(quote! { pub }),
            Visibility::Restricted(path) => {
                let path = parse_str::<Path>(path).map_err(|_| {
                    Error::SourceSyntaxMismatch("source visibility path is not Rust path")
                })?;
                Ok(quote! { pub(in #path) })
            }
        }
    }
}
