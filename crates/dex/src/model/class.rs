use super::access_flags::AccessFlags;
use super::annotation::AnnotationsDirectory;
use super::code::CodeItem;
use super::encoded_value::EncodedValue;
use super::field::FieldIdx;
use super::method::{MethodId, MethodIdx};
use super::string::{DexString, StringIdx};
use super::types::TypeIdx;

pub const NO_INDEX: u32 = 0xFFFFFFFF;

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub interfaces: Vec<TypeIdx>,
    pub source_file: Option<StringIdx>,
    pub annotations: Option<AnnotationsDirectory>,
    pub class_data: Option<ClassData>,
    pub static_values: Vec<EncodedValue>,
}

impl ClassDef {
    pub fn find_method(
        &self,
        name: &str,
        methods: &[MethodId],
        strings: &[DexString],
    ) -> Option<&EncodedMethod> {
        let data = self.class_data.as_ref()?;
        data.direct_methods
            .iter()
            .chain(data.virtual_methods.iter())
            .find(|m| {
                let method_id = &methods[m.method.0 as usize];
                strings[method_id.name.0 as usize].as_str() == name
            })
    }

    pub fn find_method_mut(
        &mut self,
        name: &str,
        methods: &[MethodId],
        strings: &[DexString],
    ) -> Option<&mut EncodedMethod> {
        let data = self.class_data.as_mut()?;
        data.direct_methods
            .iter_mut()
            .chain(data.virtual_methods.iter_mut())
            .find(|m| {
                let method_id = &methods[m.method.0 as usize];
                strings[method_id.name.0 as usize].as_str() == name
            })
    }
}

#[derive(Debug, Clone)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

#[derive(Debug, Clone)]
pub struct EncodedField {
    pub field: FieldIdx,
    pub access_flags: AccessFlags,
}

#[derive(Debug, Clone)]
pub struct EncodedMethod {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code: Option<CodeItem>,
}

impl EncodedMethod {
    pub fn code(&self) -> Option<&CodeItem> {
        self.code.as_ref()
    }

    pub fn code_mut(&mut self) -> Option<&mut CodeItem> {
        self.code.as_mut()
    }
}
