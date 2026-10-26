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

impl MethodKind {
    /// Class data lists direct methods before virtual ones.
    pub const ALL: [MethodKind; 2] = [MethodKind::Direct, MethodKind::Virtual];

    /// The list a method with these flags belongs in: static, private and
    /// constructor methods are direct, every other method is virtual.
    pub fn of(flags: AccessFlags) -> Self {
        if flags.intersects(AccessFlags::STATIC | AccessFlags::PRIVATE | AccessFlags::CONSTRUCTOR) {
            Self::Direct
        } else {
            Self::Virtual
        }
    }
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
                .chain(&mut data.virtual_methods)
            {
                m.access_flags &= !AccessFlags::FINAL;
            }
        }
    }

    pub fn ensure_class_data(&mut self) -> &mut ClassData {
        self.class_data.get_or_insert_with(Default::default)
    }

    /// Appends a method to the list its flags select, returning where it went.
    pub fn add_method(&mut self, method: EncodedMethod) -> (MethodKind, usize) {
        let kind = MethodKind::of(method.access_flags);
        let methods = self.ensure_class_data().methods_mut(kind);
        methods.push(method);
        (kind, methods.len() - 1)
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

impl ClassData {
    pub fn methods(&self, kind: MethodKind) -> &[EncodedMethod] {
        match kind {
            MethodKind::Direct => &self.direct_methods,
            MethodKind::Virtual => &self.virtual_methods,
        }
    }

    pub fn methods_mut(&mut self, kind: MethodKind) -> &mut Vec<EncodedMethod> {
        match kind {
            MethodKind::Direct => &mut self.direct_methods,
            MethodKind::Virtual => &mut self.virtual_methods,
        }
    }

    /// Where the class declares `method`.
    pub fn find_method(&self, method: MethodIdx) -> Option<(MethodKind, usize)> {
        MethodKind::ALL.into_iter().find_map(|kind| {
            self.methods(kind)
                .iter()
                .position(|encoded| encoded.method == method)
                .map(|position| (kind, position))
        })
    }
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

/// The four id fields of a `class_def_item`, readable for any class without
/// materializing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassHeader {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub source_file: Option<StringIdx>,
}

impl ClassHeader {
    pub fn of(class: &ClassDef) -> Self {
        Self {
            class_type: class.class_type,
            access_flags: class.access_flags,
            superclass: class.superclass,
            source_file: class.source_file,
        }
    }
}
