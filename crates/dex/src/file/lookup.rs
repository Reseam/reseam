// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::cmp::Ordering;

use super::DexFile;
use crate::types::{
    FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};

impl DexFile {
    pub fn string(&self, idx: StringIdx) -> Cow<'_, str> {
        self.strings.get(idx)
    }

    pub fn type_descriptor(&self, idx: TypeIdx) -> Cow<'_, str> {
        self.string(self.type_string(idx))
    }

    pub fn proto_descriptor(&self, proto: &Prototype) -> String {
        let mut desc = String::with_capacity(64);
        desc.push('(');
        for param in &proto.parameters {
            desc.push_str(&self.type_descriptor(*param));
        }
        desc.push(')');
        desc.push_str(&self.type_descriptor(proto.return_type));
        desc
    }

    pub fn find_class_index(&self, descriptor: &str) -> Option<usize> {
        self.class_index_of(self.find_type_idx(descriptor)?)
    }

    pub fn find_string_idx(&self, s: &str) -> Option<StringIdx> {
        self.strings.find(s)
    }

    pub fn find_type_idx(&self, descriptor: &str) -> Option<TypeIdx> {
        let string_idx = self.find_string_idx(descriptor)?;
        self.types.find(&string_idx).map(|i| TypeIdx(i as u32))
    }

    pub(crate) fn find_proto_idx(&self, ret: TypeIdx, params: &[TypeIdx]) -> Option<ProtoIdx> {
        let probe = Prototype {
            shorty: StringIdx(0),
            return_type: ret,
            parameters: params.iter().copied().collect(),
        };
        self.prototypes.find(&probe).map(|i| ProtoIdx(i as u32))
    }

    pub(crate) fn find_method_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        proto: ProtoIdx,
    ) -> Option<MethodIdx> {
        self.methods
            .find(&MethodId { class, name, proto })
            .map(|i| MethodIdx(i as u32))
    }

    pub fn methods_of(
        &self,
        class: TypeIdx,
        name: Option<StringIdx>,
    ) -> impl Iterator<Item = MethodIdx> + '_ {
        self.methods
            .matching(move |m| {
                m.class
                    .cmp(&class)
                    .then_with(|| name.map_or(Ordering::Equal, |n| m.name.cmp(&n)))
            })
            .map(|i| MethodIdx(i as u32))
    }

    pub fn fields_of(
        &self,
        class: TypeIdx,
        name: Option<StringIdx>,
    ) -> impl Iterator<Item = FieldIdx> + '_ {
        self.fields
            .matching(move |f| {
                f.class
                    .cmp(&class)
                    .then_with(|| name.map_or(Ordering::Equal, |n| f.name.cmp(&n)))
            })
            .map(|i| FieldIdx(i as u32))
    }

    pub(crate) fn find_field_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        type_: TypeIdx,
    ) -> Option<FieldIdx> {
        self.fields
            .find(&FieldId { class, name, type_ })
            .map(|i| FieldIdx(i as u32))
    }

    pub fn class_index_of(&self, type_idx: TypeIdx) -> Option<usize> {
        self.classes.index_of_type(type_idx)
    }
}
