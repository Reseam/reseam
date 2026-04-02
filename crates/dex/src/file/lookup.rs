use super::DexFile;
use crate::types::class::ClassDef;
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

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

        self.class_lookup.clear();
        for (i, class) in self.classes.iter().enumerate() {
            self.class_lookup.insert(class.class_type, i);
        }

        self.proto_lookup.clear();
        for (i, proto) in self.prototypes.iter().enumerate() {
            self.proto_lookup
                .insert((proto.return_type, proto.parameters.clone()), ProtoIdx(i as u16));
        }

        self.method_lookup.clear();
        for (i, method) in self.methods.iter().enumerate() {
            self.method_lookup
                .insert((method.class, method.name, method.proto), MethodIdx(i as u32));
        }

        self.field_lookup.clear();
        for (i, field) in self.fields.iter().enumerate() {
            self.field_lookup
                .insert((field.class, field.name, field.type_), FieldIdx(i as u32));
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
        let type_idx = self.find_type_idx(descriptor)?;
        let &idx = self.class_lookup.get(&type_idx)?;
        self.classes.get(idx)
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<&mut ClassDef> {
        let type_idx = self.find_type_idx(descriptor)?;
        let &idx = self.class_lookup.get(&type_idx)?;
        self.classes.get_mut(idx)
    }

    pub fn find_class_index(&self, descriptor: &str) -> Option<usize> {
        let type_idx = self.find_type_idx(descriptor)?;
        self.class_lookup.get(&type_idx).copied()
    }

    pub fn find_string_idx(&self, s: &str) -> Option<StringIdx> {
        self.string_lookup.get(s).copied()
    }

    pub fn find_type_idx(&self, descriptor: &str) -> Option<TypeIdx> {
        let string_idx = self.string_lookup.get(descriptor)?;
        self.type_lookup.get(string_idx).copied()
    }
}
