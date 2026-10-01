// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod access_flags;
pub mod annotation;
pub mod class;
pub mod code;
pub mod code_rewrite;
pub mod debug;
pub mod encoded_value;
pub mod header;
pub mod hidden_api;
pub mod instruction;
pub(crate) mod instruction_catalogue;
pub(crate) mod instruction_encoding;
mod instruction_operands;
mod instruction_query;
mod instruction_registers;
pub mod map;
pub mod metadata;
pub mod method_handle;
pub mod register_allocation;
pub mod register_analysis;
mod register_operands;
mod register_types;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId {
    pub class: TypeIdx,
    pub name: StringIdx,
    pub type_: TypeIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MethodIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodId {
    pub class: TypeIdx,
    pub name: StringIdx,
    pub proto: ProtoIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtoIdx(pub u32);

pub type TypeList = smallvec::SmallVec<[TypeIdx; 4]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prototype {
    pub shorty: StringIdx,
    pub return_type: TypeIdx,
    pub parameters: TypeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pool {
    String,
    Type,
    Proto,
    Field,
    Method,
    CallSite,
    MethodHandle,
}

impl Pool {
    pub(crate) const ALL: [Pool; 7] = [
        Pool::String,
        Pool::Type,
        Pool::Proto,
        Pool::Field,
        Pool::Method,
        Pool::CallSite,
        Pool::MethodHandle,
    ];
}
