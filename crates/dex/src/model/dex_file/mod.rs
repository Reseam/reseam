//! High-level DEX file container APIs.

mod interning;
mod pattern;
mod search;
#[cfg(test)]
mod tests;
mod version;

use std::collections::HashMap;
use std::sync::Arc;

use super::call_site::CallSiteItem;
use super::class::ClassDef;
use super::field::FieldId;
use super::header::DexHeader;
#[cfg(test)]
use super::header::DexVersion;
use super::hidden_api::HiddenApiData;
use super::method::MethodId;
use super::method_handle::MethodHandle;
use super::proto::Prototype;
use super::string::{DexString, StringIdx};
use super::types::TypeIdx;
use crate::error::DexError;

pub use pattern::{InstructionPattern, OpcodeMatcher};
pub use search::MethodMatch;

/// In-memory representation of a DEX file and its derived lookup tables.
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
    pub hidden_api: Option<HiddenApiData>,
    pub raw: Option<Arc<[u8]>>,
    pub(crate) lazy_class_data_offsets: Option<Vec<u32>>,
    string_lookup: HashMap<String, StringIdx>,
    type_lookup: HashMap<StringIdx, TypeIdx>,
}

#[cfg(test)]
fn empty_test_header() -> DexHeader {
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
    }
}

impl DexFile {
    /// Creates an empty DEX container backed by the supplied header.
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

    /// Returns the retained raw bytes when parsing kept the original buffer.
    pub fn raw_buffer(&self) -> Option<&[u8]> {
        self.raw.as_deref()
    }

    /// Reports whether class data can still be resolved lazily.
    pub fn is_lazy(&self) -> bool {
        self.lazy_class_data_offsets.is_some()
    }

    /// Resolves one lazily deferred `class_data_item` in place.
    pub fn resolve_class_data(&mut self, class_idx: usize) -> crate::error::Result<bool> {
        let offsets = match self.lazy_class_data_offsets.as_ref() {
            Some(offsets) => offsets,
            None => return Ok(false),
        };

        if class_idx >= offsets.len() || class_idx >= self.classes.len() {
            return Err(DexError::IndexOutOfBounds {
                index_type: "class",
                index: class_idx as u32,
                table_size: self.classes.len() as u32,
            });
        }

        let offset = offsets[class_idx];
        if offset == 0 || self.classes[class_idx].class_data.is_some() {
            return Ok(false);
        }

        let raw = self.raw.as_ref().ok_or(DexError::InvalidOffset {
            section: "lazy resolve requires raw buffer",
            offset,
            file_size: 0,
        })?;
        let class_data = crate::reader::class_reader::read_class_data_at(raw, offset as usize)?;
        self.classes[class_idx].class_data = Some(class_data);
        Ok(true)
    }

    /// Resolves all deferred class data and disables lazy mode.
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
