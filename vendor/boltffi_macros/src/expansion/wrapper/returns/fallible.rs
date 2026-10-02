use boltffi_binding::{
    DirectValueType, ErrorDecl, Native, OutOfRust, ReadPlan, ReturnDecl, ReturnPlan, Wasm32,
};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::expansion::{
    contract::Expansion,
    error::Error,
    rust_api,
    wrapper::{self, names},
};

use super::{RustInvocation, Tokens, closure, encoded, handle};

pub struct Input<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> {
    returns: &'lowered ReturnDecl<S, OutOfRust>,
    error: &'lowered ErrorDecl<S, OutOfRust>,
    source: rust_api::Return<'lowered>,
    invocation: RustInvocation,
    expansion: &'expansion Expansion<'lowered, S>,
}

pub struct SuccessInput<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> {
    returns: &'lowered ReturnDecl<S, OutOfRust>,
    source: rust_api::Fallible<'lowered>,
    owner: Ident,
    span: Span,
    expansion: &'expansion Expansion<'lowered, S>,
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> SuccessInput<'expansion, 'lowered, S> {
    pub fn new(
        returns: &'lowered ReturnDecl<S, OutOfRust>,
        source: rust_api::Fallible<'lowered>,
        owner: Ident,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        let span = owner.span();
        Self {
            returns,
            source,
            owner,
            span,
            expansion,
        }
    }
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> Input<'expansion, 'lowered, S> {
    pub fn new(
        returns: &'lowered ReturnDecl<S, OutOfRust>,
        error: &'lowered ErrorDecl<S, OutOfRust>,
        source: rust_api::Return<'lowered>,
        invocation: RustInvocation,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            returns,
            error,
            source,
            invocation,
            expansion,
        }
    }
}

impl<'expansion, 'lowered> Input<'expansion, 'lowered, Native> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.error {
            ErrorDecl::EncodedViaReturnSlot { codec, shape, .. } => EncodedError::new(
                self.returns,
                codec,
                *shape,
                self.source.fallible()?,
                self.invocation,
                self.expansion,
            )
            .tokens(),
            ErrorDecl::StatusViaReturnSlot { .. } => {
                Err(Error::UnsupportedExpansion("status error return"))
            }
            ErrorDecl::StatusViaOutPointer { .. } => {
                Err(Error::UnsupportedExpansion("status error out-pointer"))
            }
            ErrorDecl::EncodedViaOutPointer { .. } => {
                Err(Error::UnsupportedExpansion("encoded error out-pointer"))
            }
            ErrorDecl::None(_) => Err(Error::UnsupportedExpansion("missing error channel")),
            _ => Err(Error::UnsupportedExpansion("unknown error channel")),
        }
    }
}

impl<'expansion, 'lowered> Input<'expansion, 'lowered, Wasm32> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.error {
            ErrorDecl::EncodedViaReturnSlot { codec, shape, .. } => EncodedError::new(
                self.returns,
                codec,
                *shape,
                self.source.fallible()?,
                self.invocation,
                self.expansion,
            )
            .tokens(),
            ErrorDecl::StatusViaReturnSlot { .. } => {
                Err(Error::UnsupportedExpansion("status error return"))
            }
            ErrorDecl::StatusViaOutPointer { .. } => {
                Err(Error::UnsupportedExpansion("status error out-pointer"))
            }
            ErrorDecl::EncodedViaOutPointer { .. } => {
                Err(Error::UnsupportedExpansion("encoded error out-pointer"))
            }
            ErrorDecl::None(_) => Err(Error::UnsupportedExpansion("missing error channel")),
            _ => Err(Error::UnsupportedExpansion("unknown error channel")),
        }
    }
}

struct EncodedError<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> {
    returns: &'lowered ReturnDecl<S, OutOfRust>,
    error_codec: &'lowered ReadPlan,
    error_shape: S::BufferShape,
    source: rust_api::Fallible<'lowered>,
    invocation: RustInvocation,
    expansion: &'expansion Expansion<'lowered, S>,
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> EncodedError<'expansion, 'lowered, S> {
    fn new(
        returns: &'lowered ReturnDecl<S, OutOfRust>,
        error_codec: &'lowered ReadPlan,
        error_shape: S::BufferShape,
        source: rust_api::Fallible<'lowered>,
        invocation: RustInvocation,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            returns,
            error_codec,
            error_shape,
            source,
            invocation,
            expansion,
        }
    }

    fn finish(
        invocation: RustInvocation,
        error_ident: Ident,
        error: encoded::Tokens,
        empty_error: encoded::Tokens,
        success: SuccessTokens,
    ) -> Result<Tokens, Error> {
        let return_type = error.return_type().clone();
        let error_value = error.value();
        let empty_error_value = empty_error.value();
        let (success_items, success_ffi_parameters, success_pattern, success_body) =
            success.into_parts();
        let RustInvocation {
            span,
            call,
            conversions,
            writebacks,
            ..
        } = invocation;
        let result = names::Locals::new(span).result();
        let result_value = if writebacks.is_empty() {
            quote! { #call }
        } else {
            quote! {
                {
                    let #result = #call;
                    #(#writebacks)*
                    #result
                }
            }
        };

        Ok(Tokens {
            items: success_items,
            ffi_parameters: success_ffi_parameters,
            return_type,
            body: quote! {
                #(#conversions)*
                match #result_value {
                    Ok(#success_pattern) => {
                        #success_body
                        #empty_error_value
                    }
                    Err(#error_ident) => {
                        #error_value
                    }
                }
            },
        })
    }
}

impl<'expansion, 'lowered> EncodedError<'expansion, 'lowered, Native> {
    fn tokens(self) -> Result<Tokens, Error> {
        let error_ident = names::Locals::new(self.invocation.span).error();
        let error = encoded::Input::new(
            self.error_codec,
            self.error_shape,
            error_ident.clone(),
            self.expansion,
        )
        .render()?;
        let empty_error = encoded::Empty::<Native>::new(self.error_shape).render()?;
        let success = SuccessInput::new(
            self.returns,
            self.source,
            self.invocation.owner.clone(),
            self.expansion,
        )
        .render()?;
        Self::finish(self.invocation, error_ident, error, empty_error, success)
    }
}

impl<'expansion, 'lowered> EncodedError<'expansion, 'lowered, Wasm32> {
    fn tokens(self) -> Result<Tokens, Error> {
        let error_ident = names::Locals::new(self.invocation.span).error();
        let error = encoded::Input::new(
            self.error_codec,
            self.error_shape,
            error_ident.clone(),
            self.expansion,
        )
        .render()?;
        let empty_error = encoded::Empty::<Wasm32>::new(self.error_shape).render()?;
        let success = SuccessInput::new(
            self.returns,
            self.source,
            self.invocation.owner.clone(),
            self.expansion,
        )
        .render()?;
        Self::finish(self.invocation, error_ident, error, empty_error, success)
    }
}

impl<'expansion, 'lowered, S: boltffi_binding::SurfaceLower> SuccessInput<'expansion, 'lowered, S> {
    fn direct(&self) -> Result<Option<SuccessTokens>, Error> {
        let locals = names::Locals::new(self.span);
        let success_ident = locals.success();
        let tokens = match self.returns.plan() {
            ReturnPlan::Void => SuccessTokens {
                items: Vec::new(),
                ffi_parameters: Vec::new(),
                pattern: quote! { () },
                body: TokenStream::new(),
            },
            ReturnPlan::DirectViaOutPointer {
                ty: DirectValueType::Primitive(primitive),
            } => {
                let out = locals.return_out();
                let ty = wrapper::type_ref::primitive(*primitive)?;
                SuccessTokens {
                    items: Vec::new(),
                    ffi_parameters: vec![quote! { #out: *mut #ty }],
                    pattern: quote! { #success_ident },
                    body: quote! {
                        if !#out.is_null() {
                            unsafe {
                                ::core::ptr::write(#out, #success_ident);
                            }
                        }
                    },
                }
            }
            ReturnPlan::DirectViaOutPointer { .. } => {
                let out = locals.return_out();
                let ok = self.source.ok_written_type()?;
                SuccessTokens {
                    items: Vec::new(),
                    ffi_parameters: vec![quote! {
                        #out: *mut <#ok as ::boltffi::__private::Passable>::Out
                    }],
                    pattern: quote! { #success_ident },
                    body: quote! {
                        if !#out.is_null() {
                            unsafe {
                                ::core::ptr::write(
                                    #out,
                                    <#ok as ::boltffi::__private::Passable>::pack(#success_ident),
                                );
                            }
                        }
                    },
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(tokens))
    }

    fn encoded(&self, encoded: encoded::Tokens) -> SuccessTokens {
        let locals = names::Locals::new(self.span);
        let success_ident = locals.success();
        let out = locals.return_out();
        let out_ty = encoded.return_type_without_arrow();
        let encoded_value = encoded.value();
        SuccessTokens {
            items: Vec::new(),
            ffi_parameters: vec![quote! { #out: *mut #out_ty }],
            pattern: quote! { #success_ident },
            body: quote! {
                if !#out.is_null() {
                    unsafe {
                        ::core::ptr::write(#out, #encoded_value);
                    }
                }
            },
        }
    }

    fn handle(&self, handle: handle::ValueTokens) -> SuccessTokens {
        let locals = names::Locals::new(self.span);
        let success_ident = locals.success();
        let out = locals.return_out();
        let out_ty = handle.ty();
        let handle_value = handle.value();
        SuccessTokens {
            items: Vec::new(),
            ffi_parameters: vec![quote! { #out: *mut #out_ty }],
            pattern: quote! { #success_ident },
            body: quote! {
                if !#out.is_null() {
                    unsafe {
                        ::core::ptr::write(#out, #handle_value);
                    }
                }
            },
        }
    }

    fn closure(&self, writer: closure::WriteTokens) -> SuccessTokens {
        let success_ident = names::Locals::new(self.span).success();
        let (items, ffi_parameters, body) = writer.into_parts();
        SuccessTokens {
            items,
            ffi_parameters,
            pattern: quote! { #success_ident },
            body,
        }
    }
}

impl<'expansion, 'lowered> SuccessInput<'expansion, 'lowered, Native> {
    pub fn render(self) -> Result<SuccessTokens, Error> {
        if let Some(tokens) = self.direct()? {
            return Ok(tokens);
        }
        let success_ident = names::Locals::new(self.span).success();
        match self.returns.plan() {
            ReturnPlan::EncodedViaOutPointer { codec, shape, .. } => {
                let encoded =
                    encoded::Input::new(codec, *shape, success_ident, self.expansion).render()?;
                Ok(self.encoded(encoded))
            }
            ReturnPlan::HandleViaOutPointer {
                target,
                carrier,
                presence,
            } => {
                let handle_return = self.source.ok_handle_return(target, *presence)?;
                let handle = handle::ValueInput::new(
                    self.expansion,
                    target,
                    *carrier,
                    *presence,
                    success_ident,
                    handle_return,
                )
                .render()?;
                Ok(self.handle(handle))
            }
            ReturnPlan::ClosureViaOutPointer(closure) => {
                let source_closure = self.source.ok_closure(closure.presence())?;
                let writer = closure::WriteInput::success(
                    closure,
                    source_closure,
                    success_ident,
                    self.owner.clone(),
                    self.expansion,
                )
                .render()?;
                Ok(self.closure(writer))
            }
            _ => Err(Error::UnsupportedExpansion("fallible return shape")),
        }
    }
}

impl<'expansion, 'lowered> SuccessInput<'expansion, 'lowered, Wasm32> {
    pub fn render(self) -> Result<SuccessTokens, Error> {
        if let Some(tokens) = self.direct()? {
            return Ok(tokens);
        }
        let success_ident = names::Locals::new(self.span).success();
        match self.returns.plan() {
            ReturnPlan::EncodedViaOutPointer { codec, shape, .. } => {
                let encoded =
                    encoded::Input::new(codec, *shape, success_ident, self.expansion).render()?;
                Ok(self.encoded(encoded))
            }
            ReturnPlan::HandleViaOutPointer {
                target,
                carrier,
                presence,
            } => {
                let handle_return = self.source.ok_handle_return(target, *presence)?;
                let handle = handle::ValueInput::new(
                    self.expansion,
                    target,
                    *carrier,
                    *presence,
                    success_ident,
                    handle_return,
                )
                .render()?;
                Ok(self.handle(handle))
            }
            ReturnPlan::ClosureViaOutPointer(closure) => {
                let source_closure = self.source.ok_closure(closure.presence())?;
                let writer = closure::WriteInput::success(
                    closure,
                    source_closure,
                    success_ident,
                    self.owner.clone(),
                    self.expansion,
                )
                .render()?;
                Ok(self.closure(writer))
            }
            _ => Err(Error::UnsupportedExpansion("fallible return shape")),
        }
    }
}

pub struct SuccessTokens {
    items: Vec<TokenStream>,
    ffi_parameters: Vec<TokenStream>,
    pattern: TokenStream,
    body: TokenStream,
}

impl SuccessTokens {
    pub fn into_parts(self) -> (Vec<TokenStream>, Vec<TokenStream>, TokenStream, TokenStream) {
        (self.items, self.ffi_parameters, self.pattern, self.body)
    }
}
