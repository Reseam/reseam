pub mod container;
mod class_ops;
mod fingerprint;
mod interning;
mod lookup;
mod pattern;
mod search;
#[cfg(test)]
mod tests;
mod version;

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{index_out_of_bounds, invalid_offset};
use crate::types::class::ClassDef;
use crate::types::header::DexHeader;
use crate::types::method_handle::{CallSiteItem, MethodHandle};
use crate::types::{DexString, FieldId, MethodId, Prototype, StringIdx, TypeIdx};

pub use fingerprint::{Fingerprint, FingerprintBuilder, FingerprintMatch};
pub use pattern::{InstructionPattern, OpcodeMatcher};
pub use search::MethodMatch;

#[derive(Debug, Clone)]
pub struct DexFile {
    pub header: DexHeader,
    pub strings: Vec<DexString>,
    pub types: Vec<StringIdx>,
    pub prototypes: Vec<Prototype>,
    pub fields: Vec<FieldId>,
    pub methods: Vec<MethodId>,
    pub classes: Vec<ClassDef>,
    pub call_sites: Vec<CallSiteItem>,
    pub method_handles: Vec<MethodHandle>,
    pub hidden_api: Option<crate::types::hidden_api::HiddenApiData>,
    pub raw: Option<Arc<[u8]>>,
    pub(crate) lazy_class_data_offsets: Option<Vec<u32>>,
    string_lookup: HashMap<String, StringIdx>,
    type_lookup: HashMap<StringIdx, TypeIdx>,
}

#[cfg(test)]
fn empty_test_header() -> DexHeader {
    use crate::types::header::DexVersion;
    DexHeader {
        version: DexVersion::V035,
        checksum: 0,
        signature: [0; 20],
        file_size: 0,
        link_size: 0,
        link_off: 0,
        map_off: 0,
        string_ids_size: 0,
        string_ids_off: 0,
        type_ids_size: 0,
        type_ids_off: 0,
        proto_ids_size: 0,
        proto_ids_off: 0,
        field_ids_size: 0,
        field_ids_off: 0,
        method_ids_size: 0,
        method_ids_off: 0,
        class_defs_size: 0,
        class_defs_off: 0,
        data_size: 0,
        data_off: 0,
        container_size: 0,
        header_offset: 0,
    }
}

impl DexFile {
    pub fn new(header: DexHeader) -> Self {
        Self {
            header,
            strings: Vec::new(),
            types: Vec::new(),
            prototypes: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            classes: Vec::new(),
            call_sites: Vec::new(),
            method_handles: Vec::new(),
            hidden_api: None,
            raw: None,
            lazy_class_data_offsets: None,
            string_lookup: HashMap::new(),
            type_lookup: HashMap::new(),
        }
    }

    pub fn raw_buffer(&self) -> Option<&[u8]> {
        self.raw.as_deref()
    }

    pub fn method_count(&self) -> usize {
        self.methods.len()
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    pub fn can_add_methods(&self, count: usize) -> bool {
        self.methods.len() + count <= crate::write::MAX_POOL_SIZE
    }

    pub fn can_add_fields(&self, count: usize) -> bool {
        self.fields.len() + count <= crate::write::MAX_POOL_SIZE
    }

    pub fn can_add_types(&self, count: usize) -> bool {
        self.types.len() + count <= crate::write::MAX_POOL_SIZE
    }

    pub fn is_lazy(&self) -> bool {
        self.lazy_class_data_offsets.is_some()
    }

    pub fn resolve_class_data(&mut self, class_idx: usize) -> crate::error::Result<bool> {
        let offsets = match self.lazy_class_data_offsets.as_ref() {
            Some(offsets) => offsets,
            None => return Ok(false),
        };

        if class_idx >= offsets.len() || class_idx >= self.classes.len() {
            return Err(index_out_of_bounds(
                "class",
                class_idx as u32,
                self.classes.len() as u32,
            ));
        }

        let offset = offsets[class_idx];
        if offset == 0 || self.classes[class_idx].class_data.is_some() {
            return Ok(false);
        }

        let raw = self
            .raw
            .as_ref()
            .ok_or_else(|| invalid_offset("lazy class data", offset, 0))?;
        let class_data = crate::read::class::read_class_data_at(raw, offset as usize)?;
        self.classes[class_idx].class_data = Some(class_data);
        Ok(true)
    }

    pub fn resolve_all_class_data(&mut self) -> crate::error::Result<()> {
        if self.lazy_class_data_offsets.is_none() {
            return Ok(());
        }

        for i in 0..self.classes.len() {
            self.resolve_class_data(i)?;
        }
        self.lazy_class_data_offsets = None;
        Ok(())
    }
}
