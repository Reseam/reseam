use super::DexFile;
use crate::error::{index_out_of_bounds, invalid_descriptor, Result};
use crate::model::access_flags::AccessFlags;
use crate::model::class::{ClassData, ClassDef};
use crate::model::field::{FieldId, FieldIdx};
use crate::model::method::{MethodId, MethodIdx};
use crate::model::proto::{ProtoIdx, Prototype};
use crate::model::string::{DexString, StringIdx};
use crate::model::types::TypeIdx;

impl DexFile {
    /// Rebuilds descriptor lookup tables after structural edits.
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

    /// Returns the UTF-8 contents of a string table entry.
    pub fn string(&self, idx: StringIdx) -> &str {
        self.strings[idx.0 as usize].as_str()
    }

    /// Returns a type descriptor by resolving a type index through the string table.
    pub fn type_descriptor(&self, idx: TypeIdx) -> &str {
        let string_idx = self.types[idx.0 as usize];
        self.string(string_idx)
    }

    /// Returns the class definitions owned by this file.
    pub fn classes(&self) -> &[ClassDef] {
        &self.classes
    }

    /// Finds a class by descriptor.
    pub fn find_class(&self, descriptor: &str) -> Option<&ClassDef> {
        self.classes
            .iter()
            .find(|c| self.type_descriptor(c.class_type) == descriptor)
    }

    /// Finds a mutable class reference by descriptor.
    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<&mut ClassDef> {
        let type_idx = self.find_type_idx(descriptor)?;
        self.classes.iter_mut().find(|c| c.class_type == type_idx)
    }

    /// Finds a string index by value, returning `None` if absent.
    pub fn find_string_idx(&self, s: &str) -> Option<StringIdx> {
        self.string_lookup.get(s).copied()
    }

    /// Finds a type index for a descriptor already present in the file.
    pub fn find_type_idx(&self, descriptor: &str) -> Option<TypeIdx> {
        let string_idx = self.string_lookup.get(descriptor)?;
        self.type_lookup.get(string_idx).copied()
    }

    /// Inserts a string if needed and returns its canonical index.
    pub fn intern_string(&mut self, s: &str) -> StringIdx {
        if let Some(&idx) = self.string_lookup.get(s) {
            return idx;
        }

        let idx = StringIdx(self.strings.len() as u32);
        self.strings.push(DexString::new(s.to_owned()));
        self.string_lookup.insert(s.to_owned(), idx);
        idx
    }

    /// Inserts a validated type descriptor if needed and returns its canonical index.
    pub fn intern_type(&mut self, descriptor: &str) -> TypeIdx {
        debug_assert!(crate::util::descriptor::is_type_descriptor(descriptor));

        let string_idx = self.intern_string(descriptor);
        if let Some(&idx) = self.type_lookup.get(&string_idx) {
            return idx;
        }

        let idx = TypeIdx(self.types.len() as u32);
        self.types.push(string_idx);
        self.type_lookup.insert(string_idx, idx);
        idx
    }

    /// Validates a type descriptor for APIs that accept user input.
    fn validate_type_descriptor(kind: &'static str, descriptor: &str) -> Result<()> {
        if crate::util::descriptor::is_type_descriptor(descriptor) {
            return Ok(());
        }

        Err(invalid_descriptor(kind, descriptor))
    }

    /// Inserts a method prototype descriptor if needed and returns its canonical index.
    pub fn intern_proto(&mut self, descriptor: &str) -> Result<ProtoIdx> {
        use crate::util::descriptor::{parse_method_descriptor, shorty_from_descriptor};

        let (param_strs, ret_str) = parse_method_descriptor(descriptor)
            .ok_or_else(|| invalid_descriptor("method descriptor", descriptor))?;

        let return_type = self.intern_type(ret_str);
        let parameters: Vec<TypeIdx> = param_strs
            .iter()
            .copied()
            .map(|p| self.intern_type(p))
            .collect();
        let shorty_str = shorty_from_descriptor(descriptor)
            .ok_or_else(|| invalid_descriptor("method shorty descriptor", descriptor))?;
        let shorty = self.intern_string(&shorty_str);

        for (i, proto) in self.prototypes.iter().enumerate() {
            if proto.return_type == return_type && proto.parameters == parameters {
                return Ok(ProtoIdx(i as u16));
            }
        }

        let idx = ProtoIdx(self.prototypes.len() as u16);
        self.prototypes.push(Prototype {
            shorty,
            return_type,
            parameters,
        });
        Ok(idx)
    }

    /// Inserts a method reference if needed and returns its canonical index.
    pub fn intern_method(&mut self, class: &str, name: &str, proto: &str) -> Result<MethodIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;

        let class_idx = self.intern_type(class);
        let name_idx = self.intern_string(name);
        let proto_idx = self.intern_proto(proto)?;

        for (i, method) in self.methods.iter().enumerate() {
            if method.class == class_idx && method.name == name_idx && method.proto == proto_idx {
                return Ok(MethodIdx(i as u32));
            }
        }

        let idx = MethodIdx(self.methods.len() as u32);
        self.methods.push(MethodId {
            class: class_idx,
            proto: proto_idx,
            name: name_idx,
        });
        Ok(idx)
    }

    /// Inserts a field reference if needed and returns its canonical index.
    pub fn intern_field(&mut self, class: &str, name: &str, type_: &str) -> Result<FieldIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;
        Self::validate_type_descriptor("field descriptor", type_)?;

        let class_idx = self.intern_type(class);
        let name_idx = self.intern_string(name);
        let type_idx = self.intern_type(type_);

        for (i, field) in self.fields.iter().enumerate() {
            if field.class == class_idx && field.name == name_idx && field.type_ == type_idx {
                return Ok(FieldIdx(i as u32));
            }
        }

        let idx = FieldIdx(self.fields.len() as u32);
        self.fields.push(FieldId {
            class: class_idx,
            type_: type_idx,
            name: name_idx,
        });
        Ok(idx)
    }

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
