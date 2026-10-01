// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexFile;
use crate::error::Result;
use crate::types::TypeIdx;
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, ClassDef};

impl DexFile {
    pub fn add_class(&mut self, class: ClassDef) -> usize {
        self.classes_mut().push(class)
    }

    pub fn remove_class(&mut self, type_: TypeIdx) -> Result<Option<ClassDef>> {
        let Some(pos) = self.class_index_of(type_) else {
            return Ok(None);
        };
        let options = self.parse_options;
        self.classes_mut().remove(pos, options).map(Some)
    }

    pub fn create_class(
        &mut self,
        descriptor: &str,
        access_flags: AccessFlags,
        superclass: Option<&str>,
    ) -> Result<usize> {
        Self::validate_type_descriptor("class descriptor", descriptor)?;
        if let Some(sc) = superclass {
            Self::validate_type_descriptor("superclass descriptor", sc)?;
        }

        let class_type = self.intern_type(descriptor)?;
        let superclass = superclass.map(|sc| self.intern_type(sc)).transpose()?;
        Ok(self.add_class(ClassDef {
            class_type,
            access_flags,
            superclass,
            interfaces: crate::types::TypeList::new(),
            source_file: None,
            annotations: None,
            class_data: Some(Box::new(ClassData::default())),
            static_values: std::collections::BTreeMap::new(),
        }))
    }

    pub fn set_superclass(&mut self, class_idx: usize, superclass: &str) -> Result<()> {
        Self::validate_type_descriptor("superclass descriptor", superclass)?;
        let type_idx = self.intern_type(superclass)?;
        self.class_mut(class_idx)?.superclass = Some(type_idx);
        Ok(())
    }
}
