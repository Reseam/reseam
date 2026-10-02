use boltffi_ast::ConstantDef;
use boltffi_binding::{ConstantDecl, ConstantValueDecl};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Path;

use crate::expansion::{
    contract::{DeclarationPair, Expansion},
    error::Error,
    rust_api,
    wrapper::{export, names},
};

pub struct Constant<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> {
    pair: DeclarationPair<'lowered, ConstantDef, ConstantDecl<S>>,
    expansion: &'expansion Expansion<'lowered, S>,
    owner: Option<AssociatedOwner<'lowered>>,
}

struct AssociatedOwner<'source> {
    callable: rust_api::CallableOwner<'source>,
    rust_type: TokenStream,
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> Constant<'expansion, 'lowered, S> {
    pub fn new(
        pair: DeclarationPair<'lowered, ConstantDef, ConstantDecl<S>>,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            pair,
            expansion,
            owner: None,
        }
    }

    pub fn with_owner(mut self, owner: rust_api::CallableOwner<'lowered>, rust_type: Path) -> Self {
        self.owner = Some(AssociatedOwner {
            callable: owner,
            rust_type: quote! { #rust_type },
        });
        self
    }

    fn constant_ident(source: &ConstantDef) -> Result<syn::Ident, Error> {
        names::SourceSpelling::new(&source.name)
            .ident("source constant name is not a Rust identifier")
    }
}

impl<'expansion, 'lowered> Constant<'expansion, 'lowered, boltffi_binding::Native> {
    pub fn render(self) -> Result<TokenStream, Error> {
        match self.pair.binding().value() {
            ConstantValueDecl::Inline { .. } => Ok(TokenStream::new()),
            ConstantValueDecl::Accessor { symbol, callable } => {
                let source = self.pair.source();
                let constant = Self::constant_ident(source)?;
                let source_callable = self.owner.as_ref().map_or_else(
                    || rust_api::Callable::constant(source),
                    |owner| rust_api::Callable::associated_constant(source, owner.callable),
                );
                let rust_call = match self.owner {
                    Some(owner) => export::RustCall::associated_constant(owner.rust_type, constant),
                    None => export::RustCall::constant(constant),
                };
                export::Export::<boltffi_binding::Native>::new(
                    symbol,
                    callable,
                    source_callable,
                    rust_call,
                    export::ReceiverTokens::none(),
                    rust_api::VisibilityTokens::new(&source.source.visibility).into_tokens()?,
                    self.expansion,
                )
                .render()
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown constant value delivery",
            )),
        }
    }
}

impl<'expansion, 'lowered> Constant<'expansion, 'lowered, boltffi_binding::Wasm32> {
    pub fn render(self) -> Result<TokenStream, Error> {
        match self.pair.binding().value() {
            ConstantValueDecl::Inline { .. } => Ok(TokenStream::new()),
            ConstantValueDecl::Accessor { symbol, callable } => {
                let source = self.pair.source();
                let constant = Self::constant_ident(source)?;
                let source_callable = self.owner.as_ref().map_or_else(
                    || rust_api::Callable::constant(source),
                    |owner| rust_api::Callable::associated_constant(source, owner.callable),
                );
                let rust_call = match self.owner {
                    Some(owner) => export::RustCall::associated_constant(owner.rust_type, constant),
                    None => export::RustCall::constant(constant),
                };
                export::Export::<boltffi_binding::Wasm32>::new(
                    symbol,
                    callable,
                    source_callable,
                    rust_call,
                    export::ReceiverTokens::none(),
                    rust_api::VisibilityTokens::new(&source.source.visibility).into_tokens()?,
                    self.expansion,
                )
                .render()
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown constant value delivery",
            )),
        }
    }
}
