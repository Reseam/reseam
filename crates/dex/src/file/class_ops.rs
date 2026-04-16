// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexFile;
use crate::error::{index_out_of_bounds, Result};
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, ClassDef};
use crate::types::TypeIdx;

impl DexFile {
    pub fn add_class(&mut self, class: ClassDef) {
        let idx = self.classes.len();
        let type_idx = class.class_type;
        self.classes.push(class);
        self.class_lookup.insert(type_idx, idx);
    }

    pub fn remove_class(&mut self, type_: TypeIdx) -> Option<ClassDef> {
        let pos = self.class_lookup.remove(&type_)?;
        let removed = self.classes.remove(pos);
        // Rebuild class_lookup since indices shifted
        self.class_lookup.clear();
        for (i, c) in self.classes.iter().enumerate() {
            self.class_lookup.insert(c.class_type, i);
        }
        Some(removed)
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

        let class_type = self.intern_type(descriptor);
        let superclass_idx = superclass.map(|sc| self.intern_type(sc));

        let idx = self.classes.len();
        let class = ClassDef {
            class_type,
            access_flags,
            superclass: superclass_idx,
            interfaces: Vec::new(),
            source_file: None,
            annotations: None,
            class_data: Some(ClassData {
                static_fields: Vec::new(),
                instance_fields: Vec::new(),
                direct_methods: Vec::new(),
                virtual_methods: Vec::new(),
            }),
            static_values: Vec::new(),
        };
        self.class_lookup.insert(class_type, idx);
        self.classes.push(class);
        Ok(idx)
    }

    pub fn set_superclass(&mut self, class_idx: usize, superclass: &str) -> Result<()> {
        if class_idx >= self.classes.len() {
            return Err(index_out_of_bounds(
                "class",
                class_idx as u32,
                self.classes.len() as u32,
            ));
        }
        Self::validate_type_descriptor("superclass descriptor", superclass)?;
        let type_idx = self.intern_type(superclass);
        self.classes[class_idx].superclass = Some(type_idx);
        Ok(())
    }

    pub fn superclass_chain(&self, class_idx: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut current = class_idx;

        loop {
            let superclass_type = match self.classes.get(current).and_then(|c| c.superclass) {
                Some(t) => t,
                None => break,
            };

            match self.class_lookup.get(&superclass_type) {
                Some(&pos) => {
                    if chain.contains(&pos) {
                        break;
                    }
                    chain.push(pos);
                    current = pos;
                }
                None => break,
            }
        }

        chain
    }
}
