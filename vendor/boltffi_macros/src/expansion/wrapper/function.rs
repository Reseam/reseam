use boltffi_ast::{FunctionDef, Path, PathRoot};
use boltffi_binding::{ExecutionDecl, FunctionDecl};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, parse_str};

use crate::expansion::{
    contract::{DeclarationPair, Expansion},
    error::Error,
    rust_api,
    wrapper::names,
};

use super::export;

pub struct Function<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> {
    pair: DeclarationPair<'lowered, FunctionDef, FunctionDecl<S>>,
    expansion: &'expansion Expansion<'lowered, S>,
    rust_path: Option<TokenStream>,
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> Function<'expansion, 'lowered, S> {
    pub fn new(
        pair: DeclarationPair<'lowered, FunctionDef, FunctionDecl<S>>,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            pair,
            expansion,
            rust_path: None,
        }
    }

    pub fn with_path(mut self, path: &Path) -> Result<Self, Error> {
        self.rust_path = Some(Self::path_tokens(path)?);
        Ok(self)
    }

    fn function_ident(source: &FunctionDef) -> Result<Ident, Error> {
        names::SourceSpelling::new(&source.name)
            .ident("source function name is not a Rust identifier")
    }

    fn rust_call(&self, function_ident: Ident) -> export::RustCall {
        match &self.rust_path {
            Some(path) => export::RustCall::function_path(function_ident, path.clone()),
            None => export::RustCall::function(function_ident),
        }
    }

    fn path_tokens(path: &Path) -> Result<TokenStream, Error> {
        let prefix = match path.root {
            PathRoot::Relative => TokenStream::new(),
            PathRoot::Crate => quote! { crate:: },
            PathRoot::Self_ => quote! { self:: },
            PathRoot::Super(levels) => {
                let parents =
                    std::iter::repeat_n(quote! { super }, levels.get()).collect::<Vec<_>>();
                quote! { #(#parents)::*:: }
            }
            PathRoot::Absolute => quote! { :: },
        };
        let segments = path
            .segments
            .iter()
            .map(|segment| {
                if !segment.arguments.is_empty() {
                    return Err(Error::UnsupportedExpansion("generic function path"));
                }
                parse_str::<Ident>(segment.name.as_str()).map_err(|_| {
                    Error::SourceSyntaxMismatch("function path segment is not Rust syntax")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { #prefix #(#segments)::* })
    }
}

impl<'expansion, 'lowered> Function<'expansion, 'lowered, boltffi_binding::Native> {
    pub fn render(self) -> Result<TokenStream, Error> {
        let function = self.pair.binding();
        let source = self.pair.source();
        let source_signature = rust_api::Callable::function(source);
        let function_ident = Self::function_ident(source)?;
        let rust_call = self.rust_call(function_ident);
        let visibility =
            rust_api::VisibilityTokens::new(&source.source.visibility).into_tokens()?;
        if matches!(
            function.callable().execution(),
            ExecutionDecl::Asynchronous(_)
        ) {
            return crate::expansion::wrapper::async_call::Input::new(
                function,
                source_signature,
                rust_call,
                visibility,
                self.expansion,
            )
            .render();
        }

        export::Export::<boltffi_binding::Native>::new(
            function.symbol(),
            function.callable(),
            source_signature,
            rust_call,
            export::ReceiverTokens::none(),
            visibility,
            self.expansion,
        )
        .render()
    }
}

impl<'expansion, 'lowered> Function<'expansion, 'lowered, boltffi_binding::Wasm32> {
    pub fn render(self) -> Result<TokenStream, Error> {
        let function = self.pair.binding();
        let source = self.pair.source();
        let source_signature = rust_api::Callable::function(source);
        let function_ident = Self::function_ident(source)?;
        let rust_call = self.rust_call(function_ident);
        let visibility =
            rust_api::VisibilityTokens::new(&source.source.visibility).into_tokens()?;
        if matches!(
            function.callable().execution(),
            ExecutionDecl::Asynchronous(_)
        ) {
            return crate::expansion::wrapper::async_call::Input::new(
                function,
                source_signature,
                rust_call,
                visibility,
                self.expansion,
            )
            .render();
        }

        export::Export::<boltffi_binding::Wasm32>::new(
            function.symbol(),
            function.callable(),
            source_signature,
            rust_call,
            export::ReceiverTokens::none(),
            visibility,
            self.expansion,
        )
        .render()
    }
}
