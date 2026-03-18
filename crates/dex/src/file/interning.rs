use super::DexFile;
use crate::error::{invalid_descriptor, Result};
use crate::types::{DexString, FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx};

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

    pub(crate) fn validate_type_descriptor(kind: &'static str, descriptor: &str) -> Result<()> {
        if crate::util::descriptor::is_type_descriptor(descriptor) {
            return Ok(());
        }

        Err(invalid_descriptor(kind, descriptor))
    }
}
