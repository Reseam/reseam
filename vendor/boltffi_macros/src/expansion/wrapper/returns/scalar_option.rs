use boltffi_binding::Primitive;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Type;

use crate::expansion::{
    error::Error,
    wrapper::{names, returns::Tokens, scalar_option::WasmScalar},
};

pub struct FailureInput {
    primitive: Primitive,
}
pub struct Empty {
    primitive: Primitive,
}
pub struct Input {
    primitive: Primitive,
    value: syn::Ident,
    enum_payload: bool,
}

pub struct IncomingInput {
    primitive: Primitive,
    rust_type: Type,
    value: TokenStream,
}

impl Input {
    pub fn new(primitive: Primitive, value: syn::Ident) -> Self {
        Self {
            primitive,
            value,
            enum_payload: false,
        }
    }

    pub fn enum_payload(primitive: Primitive, value: syn::Ident) -> Self {
        Self {
            primitive,
            value,
            enum_payload: true,
        }
    }

    pub fn native(self) -> Result<Tokens, Error> {
        let value = self.value;
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> ::boltffi::__private::FfiBuf },
            body: quote! {
                ::boltffi::__private::FfiBuf::wire_encode(&#value)
            },
        })
    }

    pub fn wasm32(self) -> Result<Tokens, Error> {
        let value = self.value;
        let present = names::Locals::new(value.span()).value();
        if matches!(self.primitive, Primitive::F64) {
            return Ok(Tokens {
                items: Vec::new(),
                ffi_parameters: Vec::new(),
                return_type: quote! { -> f64 },
                body: quote! {
                    match #value {
                        Some(#present) => {
                            if #present.is_nan() {
                                ::boltffi::__private::write_option_f64_presence(true);
                            }
                            #present
                        }
                        None => {
                            ::boltffi::__private::write_option_f64_presence(false);
                            f64::NAN
                        }
                    }
                },
            });
        }
        let scalar = WasmScalar::new(self.primitive, present.clone());
        let return_type = scalar.carrier_type();
        let none = scalar.none();
        let some = scalar.outgoing()?;
        let some = if self.enum_payload {
            quote! {
                {
                    let #present = ::boltffi::__private::Passable::pack(#present);
                    #some
                }
            }
        } else {
            some
        };
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> #return_type },
            body: quote! {
                match #value {
                    Some(#present) => #some,
                    None => #none,
                }
            },
        })
    }
}

impl FailureInput {
    pub fn new(primitive: Primitive) -> Self {
        Self { primitive }
    }

    pub fn native(self) -> Result<TokenStream, Error> {
        let empty = Empty::new(self.primitive).native()?;
        let body = empty.body();
        Ok(quote! {
            return #body;
        })
    }

    pub fn wasm32(self) -> Result<TokenStream, Error> {
        let empty = Empty::new(self.primitive).wasm32()?;
        let body = empty.body();
        Ok(quote! {
            return #body;
        })
    }
}

impl Empty {
    pub fn new(primitive: Primitive) -> Self {
        Self { primitive }
    }

    pub fn native(self) -> Result<Tokens, Error> {
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> ::boltffi::__private::FfiBuf },
            body: quote! { ::boltffi::__private::FfiBuf::default() },
        })
    }

    pub fn wasm32(self) -> Result<Tokens, Error> {
        if matches!(self.primitive, Primitive::F64) {
            return Ok(Tokens {
                items: Vec::new(),
                ffi_parameters: Vec::new(),
                return_type: quote! { -> f64 },
                body: quote! {
                    {
                        ::boltffi::__private::write_option_f64_presence(false);
                        f64::NAN
                    }
                },
            });
        }
        let scalar = WasmScalar::new(
            self.primitive,
            names::Locals::new(Span::call_site()).value(),
        );
        let return_type = scalar.carrier_type();
        let none = scalar.none();
        Ok(Tokens {
            items: Vec::new(),
            ffi_parameters: Vec::new(),
            return_type: quote! { -> #return_type },
            body: none,
        })
    }
}

impl IncomingInput {
    pub fn new(primitive: Primitive, rust_type: Type, value: TokenStream) -> Self {
        Self {
            primitive,
            rust_type,
            value,
        }
    }

    pub fn native(self) -> Result<TokenStream, Error> {
        let rust_type = self.rust_type;
        let value = self.value;
        Ok(quote! {
            {
                let __boltffi_result = #value;
                match ::boltffi::__private::wire::decode::<#rust_type>(unsafe {
                    __boltffi_result.as_byte_slice()
                }) {
                    Ok(__boltffi_value) => __boltffi_value,
                    Err(error) => {
                        panic!("callback method optional scalar return conversion failed: {:?}", error)
                    }
                }
            }
        })
    }

    pub fn wasm32(self) -> Result<TokenStream, Error> {
        let value = self.value;
        let result = names::Locals::new(Span::call_site()).result();
        let scalar = WasmScalar::new(self.primitive, result.clone());
        let is_none = scalar.is_none();
        let some = scalar.incoming()?;
        Ok(quote! {
            {
                let #result = #value;
                if #is_none {
                    None
                } else {
                    Some(#some)
                }
            }
        })
    }
}
