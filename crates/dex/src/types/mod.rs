// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod access_flags;
pub mod annotation;
pub mod class;
pub mod code;
pub mod debug;
pub mod encoded_value;
pub mod header;
pub mod hidden_api;
pub mod instruction;
mod instruction_encoding;
mod instruction_operands;
mod instruction_query;
mod instruction_registers;
pub mod label;
pub mod map;
pub mod method_handle;
pub mod register_analysis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldId {
    pub class: TypeIdx,
    pub type_: TypeIdx,
    pub name: StringIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MethodIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodId {
    pub class: TypeIdx,
    pub proto: ProtoIdx,
    pub name: StringIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtoIdx(pub u16);

pub type TypeList = smallvec::SmallVec<[TypeIdx; 4]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prototype {
    pub shorty: StringIdx,
    pub return_type: TypeIdx,
    pub parameters: TypeList,
}
