use boltffi_binding::{CodecNode, Native, ReadPlan, Wasm32, native, wasm32};
use proc_macro2::TokenStream;
use quote::quote;

use crate::expansion::{contract::Expansion, error::Error, wrapper::encoded};

pub struct Input<'expansion, 'codec, 'lowered, S: boltffi_binding::SurfaceLower> {
    codec: &'codec CodecNode,
    shape: S::BufferShape,
    value: syn::Ident,
    value_binding: RustValueBinding,
    expansion: &'expansion Expansion<'lowered, S>,
}

impl<'expansion, 'codec, 'lowered, S: boltffi_binding::SurfaceLower>
    Input<'expansion, 'codec, 'lowered, S>
{
    pub fn new(
        codec: &'codec ReadPlan,
        shape: S::BufferShape,
        value: syn::Ident,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            codec: codec.root(),
            shape,
            value,
            value_binding: RustValueBinding::Owned,
            expansion,
        }
    }

    pub fn string(
        codec: &'codec ReadPlan,
        shape: S::BufferShape,
        value: syn::Ident,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self::new(codec, shape, value, expansion)
    }

    pub fn borrowed(
        codec: &'codec ReadPlan,
        shape: S::BufferShape,
        value: syn::Ident,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            codec: codec.root(),
            shape,
            value,
            value_binding: RustValueBinding::Borrowed,
            expansion,
        }
    }

    pub fn root(
        codec: &'codec CodecNode,
        shape: S::BufferShape,
        value: syn::Ident,
        expansion: &'expansion Expansion<'lowered, S>,
    ) -> Self {
        Self {
            codec,
            shape,
            value,
            value_binding: RustValueBinding::Owned,
            expansion,
        }
    }
}

pub struct Tokens {
    value_type: TokenStream,
    return_type: TokenStream,
    value: TokenStream,
}

impl Tokens {
    pub fn return_type(&self) -> &TokenStream {
        &self.return_type
    }

    pub fn value(&self) -> &TokenStream {
        &self.value
    }

    pub fn return_type_without_arrow(&self) -> TokenStream {
        self.value_type.clone()
    }
}

pub struct Empty<S: boltffi_binding::SurfaceLower> {
    shape: S::BufferShape,
}

impl<S: boltffi_binding::SurfaceLower> Empty<S> {
    pub fn new(shape: S::BufferShape) -> Self {
        Self { shape }
    }
}

impl<'expansion, 'codec, 'lowered> Input<'expansion, 'codec, 'lowered, Native> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.shape {
            native::BufferShape::Buffer => {
                let value = self
                    .value_binding
                    .buffer(self.codec, self.expansion, self.value)?;
                Ok(Tokens {
                    value_type: quote! { ::boltffi::__private::FfiBuf },
                    return_type: quote! { -> ::boltffi::__private::FfiBuf },
                    value,
                })
            }
            native::BufferShape::Slice | native::BufferShape::BufferPointer => {
                Err(Error::UnsupportedExpansion("native encoded return shape"))
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown native encoded return shape",
            )),
        }
    }
}

impl Empty<Native> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.shape {
            native::BufferShape::Buffer => Ok(Tokens {
                value_type: quote! { ::boltffi::__private::FfiBuf },
                return_type: quote! { -> ::boltffi::__private::FfiBuf },
                value: quote! { ::boltffi::__private::FfiBuf::default() },
            }),
            native::BufferShape::Slice | native::BufferShape::BufferPointer => {
                Err(Error::UnsupportedExpansion("native encoded return shape"))
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown native encoded return shape",
            )),
        }
    }
}

impl<'expansion, 'codec, 'lowered> Input<'expansion, 'codec, 'lowered, Wasm32> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.shape {
            wasm32::BufferShape::Packed => {
                let buffer = self
                    .value_binding
                    .buffer(self.codec, self.expansion, self.value)?;
                Ok(Tokens {
                    value_type: quote! { u64 },
                    return_type: quote! { -> u64 },
                    value: quote! { #buffer.into_packed() },
                })
            }
            wasm32::BufferShape::Slice => {
                Err(Error::UnsupportedExpansion("wasm encoded return shape"))
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown wasm encoded return shape",
            )),
        }
    }
}

impl Empty<Wasm32> {
    pub fn render(self) -> Result<Tokens, Error> {
        match self.shape {
            wasm32::BufferShape::Packed => Ok(Tokens {
                value_type: quote! { u64 },
                return_type: quote! { -> u64 },
                value: quote! { ::boltffi::__private::FfiBuf::EMPTY_PACKED },
            }),
            wasm32::BufferShape::Slice => {
                Err(Error::UnsupportedExpansion("wasm encoded return shape"))
            }
            _ => Err(Error::UnsupportedExpansion(
                "unknown wasm encoded return shape",
            )),
        }
    }
}

#[derive(Clone, Copy)]
enum RustValueBinding {
    Owned,
    Borrowed,
}

impl RustValueBinding {
    fn buffer<'lowered, S: boltffi_binding::SurfaceLower>(
        self,
        codec: &CodecNode,
        expansion: &Expansion<'lowered, S>,
        value: syn::Ident,
    ) -> Result<TokenStream, Error> {
        let value = quote! { #value };
        match self {
            Self::Owned => encoded::outgoing::Value::new(codec, expansion).buffer(value),
            Self::Borrowed => {
                encoded::outgoing::Value::new(codec, expansion).borrowed_buffer(value)
            }
        }
    }
}
