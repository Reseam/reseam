// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod bytes;
mod class_ops;
mod classes;
pub mod container;
mod fingerprint;
mod ids;
mod interning;
mod lookup;
mod pattern;
mod ref_filter;
mod scan;
mod strings;
#[cfg(test)]
mod tests;
mod version;

pub use bytes::DexBytes;
pub use classes::{ClassHeader, ClassTable, RawClassDef};
pub(crate) use ids::read_type_list;
pub use ids::{IdRecord, IdTable};
use ref_filter::RefFilter;
pub use ref_filter::{RefKey, RefQuery};
pub use strings::StringPool;

use std::borrow::Cow;

use crate::error::invalid_offset;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;
use crate::types::header::{DexHeader, ParseOptions};
use crate::types::method_handle::{CallSiteItem, MethodHandle};
use crate::types::{
    FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};

pub use fingerprint::{Fingerprint, FingerprintBuilder, FingerprintHit};
pub use pattern::{InstructionPattern, OpcodeMatcher};
pub use scan::{
    summarize_resident, InstructionHit, InstructionSite, MemberCounts, MethodHit, MethodSummary,
    MethodView,
};

/// A DEX file whose tables are views over the mapped file. Nothing is decoded
/// until it is read, and only what a patch mutates becomes resident.
#[derive(Debug, Clone)]
pub struct DexFile {
    pub header: DexHeader,
    pub strings: StringPool,
    pub types: IdTable<StringIdx>,
    pub prototypes: IdTable<Prototype>,
    pub fields: IdTable<FieldId>,
    pub methods: IdTable<MethodId>,
    pub classes: ClassTable,
    pub call_sites: Vec<CallSiteItem>,
    pub method_handles: Vec<MethodHandle>,
    pub hidden_api: Option<crate::types::hidden_api::HiddenApiData>,
    pub raw: Option<DexBytes>,
    pub(crate) parse_options: ParseOptions,
    ref_filter: std::sync::OnceLock<RefFilter>,
    dirty: bool,
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
            strings: StringPool::default(),
            types: IdTable::default(),
            prototypes: IdTable::default(),
            fields: IdTable::default(),
            methods: IdTable::default(),
            classes: ClassTable::default(),
            call_sites: Vec::new(),
            method_handles: Vec::new(),
            hidden_api: None,
            raw: None,
            parse_options: ParseOptions::default(),
            ref_filter: std::sync::OnceLock::new(),
            dirty: false,
        }
    }

    pub fn raw_buffer(&self) -> Option<&[u8]> {
        self.raw.as_ref().map(DexBytes::as_bytes)
    }

    pub fn parse_options(&self) -> &ParseOptions {
        &self.parse_options
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

    pub fn type_string(&self, idx: TypeIdx) -> StringIdx {
        self.types.get(idx.0 as usize)
    }

    pub fn proto(&self, idx: ProtoIdx) -> Prototype {
        self.prototypes.get(idx.0 as usize)
    }

    pub fn field_id(&self, idx: FieldIdx) -> FieldId {
        self.fields.get(idx.0 as usize)
    }

    pub fn method_id(&self, idx: MethodIdx) -> MethodId {
        self.methods.get(idx.0 as usize)
    }

    pub fn class_header(&self, class_idx: usize) -> ClassHeader {
        self.classes.header(class_idx)
    }

    /// The class if a patch already materialized it.
    pub fn resident_class(&self, class_idx: usize) -> Option<&ClassDef> {
        self.classes.resident(class_idx)
    }

    /// A class's static initial values, decoded from the file for classes
    /// that are not resident.
    pub fn class_static_values(
        &self,
        class_idx: usize,
    ) -> crate::error::Result<Cow<'_, [EncodedValue]>> {
        if let Some(class) = self.classes.resident(class_idx) {
            return Ok(Cow::Borrowed(&class.static_values));
        }
        let off = self
            .classes
            .raw_def(class_idx)
            .map_or(0, |def| def.static_values_off);
        if off == 0 {
            return Ok(Cow::Borrowed(&[]));
        }
        let buf = self
            .raw_buffer()
            .ok_or_else(|| invalid_offset("static values", off, 0))?;
        Ok(Cow::Owned(
            read_encoded_array_with_opts(buf, off as usize, &self.parse_options)?.0,
        ))
    }

    /// Builds the class-type index up front instead of on the first lookup.
    pub fn build_lookups(&self) {
        self.classes.index_of_type(TypeIdx(0));
    }

    /// The per-method reference filter, built on first use.
    pub(crate) fn ref_filter(&self) -> crate::error::Result<&RefFilter> {
        if let Some(filter) = self.ref_filter.get() {
            return Ok(filter);
        }
        let filter = RefFilter::build(self)?;
        Ok(self.ref_filter.get_or_init(|| filter))
    }

    pub(crate) fn invalidate_ref_filter(&mut self) {
        self.ref_filter.take();
    }

    pub fn ref_filter_heap_bytes(&self) -> u64 {
        self.ref_filter.get().map_or(0, RefFilter::heap_bytes)
    }

    /// Materializes the class for mutation and marks the DEX dirty.
    pub fn class_mut(&mut self, class_idx: usize) -> crate::error::Result<&mut ClassDef> {
        self.touch();
        self.classes.materialize(class_idx, &self.parse_options)
    }

    /// Whether any class is still a file record rather than resident.
    pub fn is_lazy(&self) -> bool {
        !self.classes.all_resident()
    }

    /// Materializes a class for reading. Returns whether it was decoded now.
    pub fn resolve_class_data(&mut self, class_idx: usize) -> crate::error::Result<bool> {
        if self.classes.is_resident(class_idx) {
            return Ok(false);
        }
        self.classes.materialize(class_idx, &self.parse_options)?;
        Ok(true)
    }

    pub fn resolve_all_class_data(&mut self) -> crate::error::Result<()> {
        self.classes.materialize_all(&self.parse_options)
    }

    /// Whether anything changed since parse or the last [`Self::mark_clean`],
    /// so the writer can copy untouched files through verbatim.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub(crate) fn touch(&mut self) {
        if !self.dirty {
            tracing::debug!(file_size = self.header.file_size, "dex marked dirty");
        }
        self.dirty = true;
    }
}
