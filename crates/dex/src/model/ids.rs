// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodHandleIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteIdx(pub u32);

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
