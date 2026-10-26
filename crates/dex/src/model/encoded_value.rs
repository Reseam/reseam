// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{FieldIdx, MethodHandleIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

/// A value as the DEX encodes it. Values compare by encoding, so floats are
/// equal exactly when their bits are: a NaN equals itself and -0.0 is not 0.0.
#[derive(Debug, Clone)]
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

impl PartialEq for EncodedValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
            (Self::Double(left), Self::Double(right)) => left.to_bits() == right.to_bits(),
            (Self::Byte(left), Self::Byte(right)) => left == right,
            (Self::Short(left), Self::Short(right)) => left == right,
            (Self::Char(left), Self::Char(right)) => left == right,
            (Self::Int(left), Self::Int(right)) => left == right,
            (Self::Long(left), Self::Long(right)) => left == right,
            (Self::MethodType(left), Self::MethodType(right)) => left == right,
            (Self::MethodHandle(left), Self::MethodHandle(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Type(left), Self::Type(right)) => left == right,
            (Self::Field(left), Self::Field(right)) | (Self::Enum(left), Self::Enum(right)) => {
                left == right
            }
            (Self::Method(left), Self::Method(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Annotation(left), Self::Annotation(right)) => left == right,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::Null, Self::Null) => true,
            _ => false,
        }
    }
}

impl Eq for EncodedValue {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAnnotation {
    pub type_: TypeIdx,
    pub elements: Vec<EncodedAnnotationElement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
