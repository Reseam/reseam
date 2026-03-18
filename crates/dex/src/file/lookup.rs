use super::DexFile;
use crate::types::class::ClassDef;
use crate::types::{StringIdx, TypeIdx};

impl DexFile {
    pub fn build_lookups(&mut self) {
        self.string_lookup.clear();
        for (i, s) in self.strings.iter().enumerate() {
            self.string_lookup
                .insert(s.as_str().to_owned(), StringIdx(i as u32));
        }

        self.type_lookup.clear();
        for (i, &desc) in self.types.iter().enumerate() {
            self.type_lookup.insert(desc, TypeIdx(i as u32));
        }
    }

    pub fn string(&self, idx: StringIdx) -> &str {
        self.strings[idx.0 as usize].as_str()
    }

    pub fn type_descriptor(&self, idx: TypeIdx) -> &str {
        let string_idx = self.types[idx.0 as usize];
        self.string(string_idx)
    }

    pub fn classes(&self) -> &[ClassDef] {
        &self.classes
    }

    pub fn find_class(&self, descriptor: &str) -> Option<&ClassDef> {
        self.classes
            .iter()
            .find(|c| self.type_descriptor(c.class_type) == descriptor)
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<&mut ClassDef> {
        let type_idx = self.find_type_idx(descriptor)?;
        self.classes.iter_mut().find(|c| c.class_type == type_idx)
    }

    pub fn find_string_idx(&self, s: &str) -> Option<StringIdx> {
        self.string_lookup.get(s).copied()
    }

    pub fn find_type_idx(&self, descriptor: &str) -> Option<TypeIdx> {
        let string_idx = self.string_lookup.get(descriptor)?;
        self.type_lookup.get(string_idx).copied()
    }
}
