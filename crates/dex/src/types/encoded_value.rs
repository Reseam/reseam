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
