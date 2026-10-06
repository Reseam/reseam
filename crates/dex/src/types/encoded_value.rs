// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::method_handle::MethodHandleIdx;
use super::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EncodedValue {
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    MethodType(ProtoIdx),
    MethodHandle(MethodHandleIdx),
    String(StringIdx),
    Type(TypeIdx),
    Field(FieldIdx),
    Method(MethodIdx),
    Enum(FieldIdx),
    Array(Vec<EncodedValue>),
    Annotation(EncodedAnnotation),
    Null,
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedAnnotation {
    pub type_: TypeIdx,
    pub elements: Vec<EncodedAnnotationElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedAnnotationElement {
    pub name: StringIdx,
    pub value: EncodedValue,
}

macro_rules! value_kinds {
    ($($kind:ident = $tag:literal, $width:literal;)*) => {
        #[derive(Clone, Copy)]
        #[repr(u8)]
        pub(crate) enum ValueKind { $($kind = $tag,)* }
        impl ValueKind {
            pub(crate) fn from_tag(tag: u8) -> Option<Self> {
                match tag { $($tag => Some(Self::$kind),)* _ => None }
            }
            pub(crate) fn max_arg(self) -> usize {
                match self { $(Self::$kind => $width,)* }
            }
        }
    };
}
value_kinds! {
    Byte = 0x00, 0;
    Short = 0x02, 1;
    Char = 0x03, 1;
    Int = 0x04, 3;
    Long = 0x06, 7;
    Float = 0x10, 3;
    Double = 0x11, 7;
    MethodType = 0x15, 3;
    MethodHandle = 0x16, 3;
    String = 0x17, 3;
    Type = 0x18, 3;
    Field = 0x19, 3;
    Method = 0x1a, 3;
    Enum = 0x1b, 3;
    Array = 0x1c, 0;
    Annotation = 0x1d, 0;
    Null = 0x1e, 0;
    Boolean = 0x1f, 1;
}
impl EncodedValue {
    pub(crate) fn kind(&self) -> ValueKind {
        match self {
            Self::Byte(..) => ValueKind::Byte,
            Self::Short(..) => ValueKind::Short,
            Self::Char(..) => ValueKind::Char,
            Self::Int(..) => ValueKind::Int,
            Self::Long(..) => ValueKind::Long,
            Self::Float(..) => ValueKind::Float,
            Self::Double(..) => ValueKind::Double,
            Self::MethodType(..) => ValueKind::MethodType,
            Self::MethodHandle(..) => ValueKind::MethodHandle,
            Self::String(..) => ValueKind::String,
            Self::Type(..) => ValueKind::Type,
            Self::Field(..) => ValueKind::Field,
            Self::Method(..) => ValueKind::Method,
            Self::Enum(..) => ValueKind::Enum,
            Self::Array(..) => ValueKind::Array,
            Self::Annotation(..) => ValueKind::Annotation,
            Self::Null => ValueKind::Null,
            Self::Boolean(..) => ValueKind::Boolean,
        }
    }
}
