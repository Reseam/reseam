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
pub mod label;
pub mod map;
pub mod method_handle;
pub mod register_analysis;

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringIdx(pub u32);

#[derive(Debug, Clone)]
pub struct DexString {
    pub value: Cow<'static, str>,
}

impl DexString {
    pub fn new(s: String) -> Self {
        Self {
            value: Cow::Owned(s),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for DexString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldIdx(pub u32);

#[derive(Debug, Clone)]
pub struct FieldId {
    pub class: TypeIdx,
    pub type_: TypeIdx,
    pub name: StringIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MethodIdx(pub u32);

#[derive(Debug, Clone)]
pub struct MethodId {
    pub class: TypeIdx,
    pub proto: ProtoIdx,
    pub name: StringIdx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtoIdx(pub u16);

#[derive(Debug, Clone)]
pub struct Prototype {
    pub shorty: StringIdx,
    pub return_type: TypeIdx,
    pub parameters: Vec<TypeIdx>,
}
