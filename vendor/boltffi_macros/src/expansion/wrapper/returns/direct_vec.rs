use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

use crate::expansion::error::Error;

use super::Tokens;

pub struct FailureInput;

pub struct Input {
    value: syn::Ident,
    rust_element: Type,
}

pub struct IncomingInput {
    rust_element: Type,
    value: TokenStream,
}

impl Input {
    pub fn new(value: syn::Ident, rust_element: Type) -> Self {
        Self {
            value,
            rust_element,
        }
    }

    pub fn native(self) -> Result<Tokens, Error> {
        let value = self.value;
        let element = self.rust_element;
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> ::boltffi::__private::FfiBuf },
            body: quote! {
                <#element as ::boltffi::__private::VecTransport>::pack_vec(#value)
            },
        })
    }

    pub fn wasm32(self) -> Result<Tokens, Error> {
        let value = self.value;
        let element = self.rust_element;
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! {},
            body: quote! {
                let __boltffi_buf = <#element as ::boltffi::__private::VecTransport>::pack_vec(#value);
                ::boltffi::__private::write_return_slot(
                    __boltffi_buf.as_ptr() as u32,
                    __boltffi_buf.len() as u32,
                    __boltffi_buf.cap() as u32,
                    __boltffi_buf.align() as u32
                );
                core::mem::forget(__boltffi_buf);
            },
        })
    }
}

impl FailureInput {
    pub fn native(self) -> Result<TokenStream, Error> {
        let empty = Tokens::empty_native();
        let body = empty.body();
        Ok(quote! {
            return #body;
        })
    }

    pub fn wasm32(self) -> Result<TokenStream, Error> {
        let empty = Tokens::empty_wasm32();
        let body = empty.body();
        Ok(quote! {
            return #body;
        })
    }
}

impl Tokens {
    pub fn empty_native() -> Self {
        Self {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> ::boltffi::__private::FfiBuf },
            body: quote! { ::boltffi::__private::FfiBuf::default() },
        }
    }

    pub fn empty_wasm32() -> Self {
        Self {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! {},
            body: TokenStream::new(),
        }
    }
}

impl IncomingInput {
    pub fn new(rust_element: Type, value: TokenStream) -> Self {
        Self {
            rust_element,
            value,
        }
    }

    pub fn native(self) -> Result<TokenStream, Error> {
        let element = self.rust_element;
        let value = self.value;
        Ok(quote! {
            {
                let __boltffi_result = #value;
                if __boltffi_result.as_ptr().is_null() || __boltffi_result.len() == 0 {
                    Vec::new()
                } else {
                    let __boltffi_byte_len = __boltffi_result.len();
                    let __boltffi_element_size =
                        ::core::mem::size_of::<<#element as ::boltffi::__private::Passable>::In>();
                    if __boltffi_byte_len % __boltffi_element_size == 0 {
                        unsafe {
                            <#element as ::boltffi::__private::VecTransport>::unpack_vec(
                                __boltffi_result.as_ptr(),
                                __boltffi_byte_len,
                            )
                        }
                    } else {
                        panic!(
                            "invalid callback method Vec<{}> return byte length {} for element size {}",
                            ::core::any::type_name::<#element>(),
                            __boltffi_byte_len,
                            __boltffi_element_size,
                        )
                    }
                }
            }
        })
    }

    pub fn wasm32(self) -> Result<TokenStream, Error> {
        let element = self.rust_element;
        let value = self.value;
        Ok(quote! {
            {
                #value;
                unsafe {
                    ::boltffi::__private::take_return_slot_vec::<#element>()
                }
            }
        })
    }
}
