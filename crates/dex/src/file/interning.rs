use super::DexFile;
use crate::error::{invalid_descriptor, Result};
use crate::types::{
    DexString, FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};

impl DexFile {
    pub fn intern_string(&mut self, s: &str) -> StringIdx {
        if let Some(&idx) = self.string_lookup.get(s) {
            return idx;
        }

        let idx = StringIdx(self.strings.len() as u32);
        self.strings.push(DexString::new(s.to_owned()));
        self.string_lookup.insert(s.to_owned(), idx);
        idx
    }

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

        let key = (return_type, parameters.clone());
        if let Some(&idx) = self.proto_lookup.get(&key) {
            return Ok(idx);
        }

        let shorty_str = shorty_from_descriptor(descriptor)
            .ok_or_else(|| invalid_descriptor("method shorty descriptor", descriptor))?;
        let shorty = self.intern_string(&shorty_str);

        let idx = ProtoIdx(self.prototypes.len() as u16);
        self.prototypes.push(Prototype {
            shorty,
            return_type,
            parameters,
        });
        self.proto_lookup.insert(key, idx);
        Ok(idx)
    }

    pub fn intern_method(&mut self, class: &str, name: &str, proto: &str) -> Result<MethodIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;

        let class_idx = self.intern_type(class);
        let name_idx = self.intern_string(name);
        let proto_idx = self.intern_proto(proto)?;

        let key = (class_idx, name_idx, proto_idx);
        if let Some(&idx) = self.method_lookup.get(&key) {
            return Ok(idx);
        }

        let idx = MethodIdx(self.methods.len() as u32);
        self.methods.push(MethodId {
            class: class_idx,
            proto: proto_idx,
            name: name_idx,
        });
        self.method_lookup.insert(key, idx);
        Ok(idx)
    }

    pub fn intern_field(&mut self, class: &str, name: &str, type_: &str) -> Result<FieldIdx> {
        Self::validate_type_descriptor("class descriptor", class)?;
        Self::validate_type_descriptor("field descriptor", type_)?;

        let class_idx = self.intern_type(class);
        let name_idx = self.intern_string(name);
        let type_idx = self.intern_type(type_);

        let key = (class_idx, name_idx, type_idx);
        if let Some(&idx) = self.field_lookup.get(&key) {
            return Ok(idx);
        }

        let idx = FieldIdx(self.fields.len() as u32);
        self.fields.push(FieldId {
            class: class_idx,
            type_: type_idx,
            name: name_idx,
        });
        self.field_lookup.insert(key, idx);
        Ok(idx)
    }

    pub(crate) fn validate_type_descriptor(kind: &'static str, descriptor: &str) -> Result<()> {
        if crate::util::descriptor::is_type_descriptor(descriptor) {
            return Ok(());
        }

        Err(invalid_descriptor(kind, descriptor))
    }
}
