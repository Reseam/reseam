// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::access_flags::AccessFlags;
use super::annotation::AnnotationsDirectory;
use super::code::CodeItem;
use super::encoded_value::EncodedValue;
use super::{FieldIdx, MethodIdx, StringIdx, TypeIdx, TypeList};

pub const NO_INDEX: u32 = 0xFFFFFFFF;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub interfaces: TypeList,
    pub source_file: Option<StringIdx>,
    pub annotations: Option<Box<AnnotationsDirectory>>,
    pub class_data: Option<Box<ClassData>>,
    pub static_values: Vec<EncodedValue>,
}

impl ClassDef {
    pub fn set_access_flags(&mut self, flags: AccessFlags) {
        self.access_flags = flags;
    }

    pub fn definal(&mut self) {
        self.access_flags &= !AccessFlags::FINAL;
        if let Some(data) = self.class_data.as_mut() {
            for m in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                m.access_flags &= !AccessFlags::FINAL;
            }
        }
    }

    pub fn ensure_class_data(&mut self) -> &mut ClassData {
        self.class_data.get_or_insert_with(Default::default)
    }

    pub fn add_direct_method(&mut self, method: EncodedMethod) {
        self.ensure_class_data().direct_methods.push(method);
    }

    pub fn add_virtual_method(&mut self, method: EncodedMethod) {
        self.ensure_class_data().virtual_methods.push(method);
    }

    pub fn add_static_field(&mut self, field: EncodedField) {
        self.ensure_class_data().static_fields.push(field);
    }

    pub fn add_instance_field(&mut self, field: EncodedField) {
        self.ensure_class_data().instance_fields.push(field);
    }

    pub fn remove_methods_by<F: FnMut(&EncodedMethod) -> bool>(&mut self, mut predicate: F) {
        if let Some(data) = self.class_data.as_mut() {
            data.direct_methods.retain(|m| !predicate(m));
            data.virtual_methods.retain(|m| !predicate(m));
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedField {
    pub field: FieldIdx,
    pub access_flags: AccessFlags,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedMethod {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code: Option<CodeItem>,
}

impl EncodedMethod {
    pub fn code(&self) -> Option<&CodeItem> {
        self.code.as_ref()
    }

    pub fn code_mut(&mut self) -> Option<&mut CodeItem> {
        self.code.as_mut()
    }

    pub fn return_early(&mut self) {
        if let Some(code) = self.code.as_mut() {
            code.return_early();
        }
    }

    pub fn return_early_int(&mut self, value: i32) {
        if let Some(code) = self.code.as_mut() {
            code.return_early_int(value);
        }
    }

    pub fn return_early_object(&mut self, value: i32) {
        if let Some(code) = self.code.as_mut() {
            code.return_early_object(value);
        }
    }

    pub fn return_early_wide(&mut self, value: i64) {
        if let Some(code) = self.code.as_mut() {
            code.return_early_wide(value);
        }
    }
}
