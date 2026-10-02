use boltffi_binding::{BuiltinType, Primitive};
use proc_macro2::TokenStream;
use quote::quote;

use crate::expansion::error::Error;

pub fn primitive(primitive: Primitive) -> Result<TokenStream, Error> {
    Ok(match primitive {
        Primitive::Bool => quote! { bool },
        Primitive::I8 => quote! { i8 },
        Primitive::U8 => quote! { u8 },
        Primitive::I16 => quote! { i16 },
        Primitive::U16 => quote! { u16 },
        Primitive::I32 => quote! { i32 },
        Primitive::U32 => quote! { u32 },
        Primitive::I64 => quote! { i64 },
        Primitive::U64 => quote! { u64 },
        Primitive::ISize => quote! { isize },
        Primitive::USize => quote! { usize },
        Primitive::F32 => quote! { f32 },
        Primitive::F64 => quote! { f64 },
        _ => return Err(Error::UnsupportedExpansion("unknown primitive")),
    })
}

pub fn builtin(kind: BuiltinType) -> Result<TokenStream, Error> {
    Ok(match kind {
        BuiltinType::Duration => quote! { ::std::time::Duration },
        BuiltinType::SystemTime => quote! { ::std::time::SystemTime },
        BuiltinType::Uuid => quote! { ::uuid::Uuid },
        BuiltinType::Url => quote! { ::url::Url },
    })
}
