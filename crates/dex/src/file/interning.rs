// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexFile;
use crate::error::{Result, invalid_descriptor};
use crate::types::{
    FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx, TypeList,
};

impl DexFile {
    pub fn intern_string(&mut self, s: &str) -> StringIdx {
        if let Some(idx) = self.strings.find(s) {
            return idx;
        }
        self.touch();
        self.strings.push(s)
    }

    /// Interns a complete type descriptor, including `V` for prototype returns.
    /// Invalid descriptors fail before changing the pools.
    pub fn intern_type(&mut self, descriptor: &str) -> Result<TypeIdx> {
        Self::validate_type_descriptor("type descriptor", descriptor)?;

        let string_idx = self.intern_string(descriptor);
        if let Some(idx) = self.find_type_idx(descriptor) {
            return Ok(idx);
        }
        self.touch();
        Ok(TypeIdx(self.types.push(string_idx) as u32))
    }

    pub fn intern_proto(&mut self, descriptor: &str) -> Result<ProtoIdx> {
        use crate::util::descriptor::{parse_method_descriptor, shorty_from_parts};

        let (param_strs, ret_str) = parse_method_descriptor(descriptor)
            .ok_or_else(|| invalid_descriptor("method descriptor", descriptor))?;
        let existing = self.find_type_idx(ret_str).and_then(|return_type| {
            let parameters = param_strs
                .iter()
                .map(|param| self.find_type_idx(param))
                .collect::<Option<TypeList>>()?;
            self.find_proto_idx(return_type, &parameters)
        });
        if let Some(idx) = existing {
            return Ok(idx);
        }

        let index = u32::try_from(self.prototypes.len())
            .map_err(|_| crate::error::invalid("prototype pool", "too many prototypes"))?;

        let return_type = self.intern_type(ret_str)?;
        let parameters: TypeList = param_strs
            .iter()
            .copied()
            .map(|p| self.intern_type(p))
            .collect::<Result<_>>()?;

        let shorty_str = shorty_from_parts(&param_strs, ret_str);
        let shorty = self.intern_string(&shorty_str);
        self.touch();
        self.prototypes.push(Prototype {
            shorty,
            return_type,
            parameters,
        });
        Ok(ProtoIdx(index))
    }

    pub fn intern_method(&mut self, class: &str, name: &str, proto: &str) -> Result<MethodIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;

        let proto_idx = self.intern_proto(proto)?;
        let class_idx = self.intern_type(class)?;
        let name_idx = self.intern_string(name);

        if let Some(idx) = self.find_method_idx(class_idx, name_idx, proto_idx) {
            return Ok(idx);
        }
        self.touch();
        Ok(MethodIdx(self.methods.push(MethodId {
            class: class_idx,
            proto: proto_idx,
            name: name_idx,
        }) as u32))
    }

    pub fn intern_field(&mut self, class: &str, name: &str, type_: &str) -> Result<FieldIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;
        Self::validate_type_descriptor("field descriptor", type_)?;

        let class_idx = self.intern_type(class)?;
        let name_idx = self.intern_string(name);
        let type_idx = self.intern_type(type_)?;

        if let Some(idx) = self.find_field_idx(class_idx, name_idx, type_idx) {
            return Ok(idx);
        }
        self.touch();
        Ok(FieldIdx(self.fields.push(FieldId {
            class: class_idx,
            type_: type_idx,
            name: name_idx,
        }) as u32))
    }

    pub(crate) fn validate_type_descriptor(kind: &'static str, descriptor: &str) -> Result<()> {
        if crate::util::descriptor::is_type_descriptor(descriptor) {
            return Ok(());
        }

        Err(invalid_descriptor(kind, descriptor))
    }
}
