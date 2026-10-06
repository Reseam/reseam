// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::data;

#[data]
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub class_descriptor: String,
    pub method_name: String,
    pub proto: String,
    pub access_flags: u32,
    pub dex_index: u32,
    pub register_count: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub instruction_count: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub descriptor: String,
    pub access_flags: u32,
    #[boltffi::default(None)]
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    #[boltffi::default(None)]
    pub source_file: Option<String>,
    pub dex_index: u32,
    pub direct_method_count: u32,
    pub virtual_method_count: u32,
    pub static_field_count: u32,
    pub instance_field_count: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub class_descriptor: String,
    pub name: String,
    pub field_type: String,
    pub access_flags: u32,
    #[boltffi::default(None)]
    pub initial_value: Option<EncodedVal>,
}

#[data]
#[derive(Debug, Clone)]
pub struct FingerprintDef {
    #[boltffi::default(None)]
    pub name: Option<String>,
    #[boltffi::default(None)]
    pub defining_class: Option<String>,
    #[boltffi::default(None)]
    pub access_flags: Option<u32>,
    #[boltffi::default(None)]
    pub return_type: Option<String>,
    #[boltffi::default(None)]
    pub parameters: Option<Vec<String>>,
    #[boltffi::default(None)]
    pub opcodes: Option<Vec<i32>>,
    #[boltffi::default(None)]
    pub strings: Option<Vec<String>>,
    #[boltffi::default(None)]
    pub literals: Option<Vec<i64>>,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct FingerprintResult {
    pub method: u32,
    pub matched_count: u32,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct InstructionHit {
    pub method: u32,
    pub index: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegisterWriters {
    pub indices: Vec<u32>,
    pub from_entry: bool,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct MethodCallSiteResult {
    pub method: u32,
    pub index: u32,
    pub target_index: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct NewMethod {
    pub name: String,
    pub proto: String,
    pub access_flags: u32,
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub instructions: Vec<Instruction>,
    pub tries: Vec<TryItem>,
    pub catch_handlers: Vec<CatchHandler>,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_idx: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    #[boltffi::default(None)]
    pub catch_all_addr: Option<u32>,
}

#[data]
#[derive(Debug, Clone)]
pub struct TypedCatch {
    pub exception_type: String,
    pub addr: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct NewField {
    pub name: String,
    pub field_type: String,
    pub access_flags: u32,
    #[boltffi::default(None)]
    pub initial_value: Option<EncodedVal>,
}

#[data]
#[derive(Debug, Clone)]
pub enum EncodedVal {
    Null,
    BoolVal(bool),
    ByteVal(i8),
    ShortVal(i16),
    CharVal(u16),
    IntVal(i32),
    LongVal(i64),
    FloatVal(f32),
    DoubleVal(f64),
    StringVal(String),
    TypeVal(String),
    ProtoVal(String),
    HandleVal(HandleRef),
    FieldVal(FieldRef),
    MethodVal(MethodRef),
    EnumVal(FieldRef),
    ArrayVal(Vec<EncodedVal>),
    AnnotationVal(AnnotationValue),
}

#[data]
#[derive(Debug, Clone)]
pub struct AnnotationValue {
    pub annotation_type: String,
    pub elements: Vec<AnnotationElement>,
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoolOrigin {
    pub dex_index: u32,
    pub index: u32,
}

#[data]
#[derive(Debug, Clone)]
pub enum HandleRef {
    Field(FieldHandleRef),
    Method(MethodHandleRef),
}

#[data]
#[derive(Debug, Clone)]
pub struct FieldHandleRef {
    #[boltffi::default(None)]
    pub origin: Option<PoolOrigin>,
    pub kind: u16,
    pub field: FieldRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct MethodHandleRef {
    #[boltffi::default(None)]
    pub origin: Option<PoolOrigin>,
    pub kind: u16,
    pub method: MethodRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct CallSiteRef {
    #[boltffi::default(None)]
    pub origin: Option<PoolOrigin>,
    pub bootstrap: HandleRef,
    pub name: String,
    pub proto: String,
    pub arguments: Vec<EncodedVal>,
}

#[data]
#[derive(Debug, Clone)]
pub struct AnnotationItem {
    pub visibility: u8,
    pub annotation_type: String,
    pub elements: Vec<AnnotationElement>,
}

#[data]
#[derive(Debug, Clone)]
pub struct AnnotationElement {
    pub name: String,
    pub value: EncodedVal,
}

#[data]
#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub key_name: String,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct ResourceScalar {
    pub kind: u8,
    pub data: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct StyleItem {
    pub name: String,
    pub value: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct MethodRef {
    pub defining_class: String,
    pub name: String,
    pub proto: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct FieldRef {
    pub defining_class: String,
    pub name: String,
    pub field_type: String,
}

pub use super::instruction_types::*;
