// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use super::access_flags::AccessFlags;
use super::annotation::AnnotationsDirectory;
use super::code::CodeItem;
use super::encoded_value::EncodedValue;
use super::{FieldIdx, MethodIdx, StringIdx, TypeIdx, TypeList};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Direct,
    Virtual,
}

pub const NO_INDEX: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub interfaces: TypeList,
    pub source_file: Option<StringIdx>,
    pub annotations: Option<Box<super::metadata::Metadata<AnnotationsDirectory>>>,
    pub class_data: Option<Box<ClassData>>,
    pub static_values: std::collections::BTreeMap<FieldIdx, EncodedValue>,
}

impl ClassDef {
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

/// A method of a class. Copies of a class share method bodies until one of
/// them changes a body, so keeping a class as it was costs only what changed.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedMethod {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code: Option<Arc<CodeItem>>,
}

impl EncodedMethod {
    pub fn code(&self) -> Option<&CodeItem> {
        self.code.as_deref()
    }

    pub fn code_mut(&mut self) -> Option<&mut CodeItem> {
        self.code.as_mut().map(Arc::make_mut)
    }
}
