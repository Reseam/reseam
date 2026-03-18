use super::DexFile;
use crate::error::{index_out_of_bounds, Result};
use crate::model::access_flags::AccessFlags;
use crate::model::class::{ClassData, ClassDef};
use crate::model::types::TypeIdx;

impl DexFile {
    /// Adds a class definition without re-sorting related tables.
    pub fn add_class(&mut self, class: ClassDef) {
        self.classes.push(class);
    }

    /// Removes the first class definition matching the supplied type index.
    pub fn remove_class(&mut self, type_: TypeIdx) -> Option<ClassDef> {
        let pos = self.classes.iter().position(|c| c.class_type == type_)?;
        Some(self.classes.remove(pos))
    }

    /// Creates a new class, interns its descriptor and optional superclass, and returns
    /// the index into `self.classes`.
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
        self.classes.push(ClassDef {
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
        });
        Ok(idx)
    }

    /// Sets the superclass of an existing class by index.
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

    /// Walks the superclass chain starting from `class_idx`, returning indices of all
    /// ancestor classes found within this DexFile.
    pub fn superclass_chain(&self, class_idx: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut current = class_idx;

        loop {
            let superclass_type = match self.classes.get(current).and_then(|c| c.superclass) {
                Some(t) => t,
                None => break,
            };

            match self
                .classes
                .iter()
                .position(|c| c.class_type == superclass_type)
            {
                Some(pos) => {
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
