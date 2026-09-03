// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;

use super::DexFile;
use crate::types::{FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx};

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
        self.prototypes.find(&probe).map(|i| ProtoIdx(i as u16))
    }

    pub(crate) fn find_method_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        proto: ProtoIdx,
    ) -> Option<MethodIdx> {
        self.methods
            .find(&MethodId { class, proto, name })
            .map(|i| MethodIdx(i as u32))
    }

    pub(crate) fn find_field_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        type_: TypeIdx,
    ) -> Option<FieldIdx> {
        self.fields
            .find(&FieldId { class, type_, name })
            .map(|i| FieldIdx(i as u32))
    }

    pub fn class_index_of(&self, type_idx: TypeIdx) -> Option<usize> {
        self.classes.index_of_type(type_idx)
    }
}
