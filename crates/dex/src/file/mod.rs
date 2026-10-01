// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod class_ops;
mod classes;
pub mod container;
mod fingerprint;
pub(crate) mod hidden_api;
pub use hidden_api::HiddenApiData;
mod ids;
mod interning;
mod items;
pub use items::{FileRecord, FileTable};
mod lookup;
mod pattern;
mod ref_filter;
mod scan;
mod strings;
#[cfg(test)]
mod tests;
mod version;

pub use classes::{ClassHeader, ClassTable, RawClassDef};
pub use ids::{IdRecord, IdTable};
pub(crate) use ids::{read_type_list, validate_type_list};
use ref_filter::RefFilter;
pub use ref_filter::{RefKey, RefQuery};
pub use reseam_storage::Bytes as DexBytes;
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

pub use fingerprint::{Fingerprint, FingerprintHit, TypePattern};
pub use pattern::{InstructionPattern, OpcodeMatcher};
pub use scan::{
    InstructionHit, InstructionSite, MemberCounts, MethodHit, MethodSummary, MethodView,
    summarize_resident,
};

/// A DEX file whose tables are views over the mapped file. Nothing is decoded
/// until it is read, and only what a patch mutates becomes resident.
#[derive(Debug, Clone)]
pub struct DexFile {
    pub(crate) header: DexHeader,
    pub(crate) strings: StringPool,
    pub(crate) types: IdTable<StringIdx>,
    pub(crate) prototypes: IdTable<Prototype>,
    pub(crate) fields: IdTable<FieldId>,
    pub(crate) methods: IdTable<MethodId>,
    pub(crate) classes: ClassTable,
    pub(crate) call_sites: FileTable<CallSiteItem>,
    pub(crate) method_handles: FileTable<MethodHandle>,
    pub(crate) hidden_api: Option<HiddenApiData>,
    pub(crate) raw: Option<DexBytes>,
    pub(crate) parse_options: ParseOptions,
    ref_filter: std::sync::OnceLock<RefFilter>,
    dirty: bool,
    write_options: crate::write::WriteOptions,
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
            call_sites: FileTable::default(),
            method_handles: FileTable::default(),
            hidden_api: None,
            raw: None,
            parse_options: ParseOptions::default(),
            ref_filter: std::sync::OnceLock::new(),
            dirty: false,
            write_options: crate::write::WriteOptions::default(),
        }
    }

    pub fn header(&self) -> &DexHeader {
        &self.header
    }

    /// Marks the DEX changed before granting mutation.
    pub fn header_mut(&mut self) -> &mut DexHeader {
        self.touch();
        &mut self.header
    }

    pub fn strings(&self) -> &StringPool {
        &self.strings
    }

    /// Marks the DEX changed before granting mutation.
    pub fn strings_mut(&mut self) -> &mut StringPool {
        self.touch();
        &mut self.strings
    }

    pub fn types(&self) -> &IdTable<StringIdx> {
        &self.types
    }

    /// Marks the DEX changed before granting mutation.
    pub fn types_mut(&mut self) -> &mut IdTable<StringIdx> {
        self.touch();
        &mut self.types
    }

    pub fn prototypes(&self) -> &IdTable<Prototype> {
        &self.prototypes
    }

    /// Marks the DEX changed before granting mutation.
    pub fn prototypes_mut(&mut self) -> &mut IdTable<Prototype> {
        self.touch();
        &mut self.prototypes
    }

    pub fn fields(&self) -> &IdTable<FieldId> {
        &self.fields
    }

    /// Marks the DEX changed before granting mutation.
    pub fn fields_mut(&mut self) -> &mut IdTable<FieldId> {
        self.touch();
        &mut self.fields
    }

    pub fn methods(&self) -> &IdTable<MethodId> {
        &self.methods
    }

    /// Marks the DEX changed before granting mutation.
    pub fn methods_mut(&mut self) -> &mut IdTable<MethodId> {
        self.touch();
        &mut self.methods
    }

    pub fn classes(&self) -> &ClassTable {
        &self.classes
    }

    /// Marks the DEX changed before granting mutation.
    pub fn classes_mut(&mut self) -> &mut ClassTable {
        self.touch();
        // Filters are indexed by class slot; insertion or removal can shift those slots.
        self.ref_filter.take();
        &mut self.classes
    }

    pub fn call_sites(&self) -> &FileTable<CallSiteItem> {
        &self.call_sites
    }

    /// Marks the DEX changed before granting mutation.
    pub fn call_sites_mut(&mut self) -> &mut FileTable<CallSiteItem> {
        self.touch();
        &mut self.call_sites
    }

    pub fn method_handles(&self) -> &FileTable<MethodHandle> {
        &self.method_handles
    }

    /// Marks the DEX changed before granting mutation.
    pub fn method_handles_mut(&mut self) -> &mut FileTable<MethodHandle> {
        self.touch();
        &mut self.method_handles
    }

    pub fn hidden_api(&self) -> Option<&HiddenApiData> {
        self.hidden_api.as_ref()
    }

    /// Marks the DEX changed before granting mutation.
    pub fn hidden_api_mut(&mut self) -> &mut Option<HiddenApiData> {
        self.touch();
        &mut self.hidden_api
    }

    pub fn raw_buffer(&self) -> Option<&[u8]> {
        self.raw.as_ref().map(DexBytes::as_bytes)
    }

    pub fn release_pages(&self) {
        if let Some(raw) = &self.raw {
            raw.release_pages();
        }
    }

    pub fn write_options(&self) -> crate::write::WriteOptions {
        self.write_options
    }

    /// Changes serialization policy and marks the DEX dirty when it changes.
    pub fn set_write_options(&mut self, options: crate::write::WriteOptions) {
        if self.write_options != options {
            self.touch();
            self.write_options = options;
        }
    }

    /// A validated view of the source link section; opaque bytes are never remapped.
    pub fn link_data(&self) -> Option<&[u8]> {
        if self.header.link_size == 0 {
            return None;
        }
        let start = self.header.link_off as usize;
        self.raw_buffer()?
            .get(start..start + self.header.link_size as usize)
    }

    pub fn parse_options(&self) -> ParseOptions {
        self.parse_options
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
    ) -> crate::error::Result<Cow<'_, std::collections::BTreeMap<FieldIdx, EncodedValue>>> {
        if let Some(class) = self.classes.resident(class_idx) {
            return Ok(Cow::Borrowed(&class.static_values));
        }
        let off = self
            .classes
            .raw_def(class_idx)
            .map_or(0, |def| def.static_values_off);
        if off == 0 {
            return Ok(Cow::Owned(std::collections::BTreeMap::new()));
        }
        let buf = self
            .raw_buffer()
            .ok_or_else(|| invalid_offset("static values", off, 0))?;
        let values = read_encoded_array_with_opts(buf, off as usize, self.parse_options)?.0;
        let fields = self
            .decode_class_fields(class_idx)?
            .ok_or_else(|| invalid_offset("static values", off, 0))?
            .0;
        if values.len() > fields.len() {
            return Err(crate::error::invalid(
                "static values",
                "more values than static fields",
            ));
        }
        Ok(Cow::Owned(
            fields
                .into_iter()
                .zip(values)
                .map(|(field, value)| (field.field, value))
                .collect(),
        ))
    }

    /// The implicit initial value of a field with no encoded static value.
    pub fn default_field_value(&self, field: FieldIdx) -> EncodedValue {
        match self
            .type_descriptor(self.field_id(field).type_)
            .as_bytes()
            .first()
        {
            Some(b'Z') => EncodedValue::Boolean(false),
            Some(b'B') => EncodedValue::Byte(0),
            Some(b'S') => EncodedValue::Short(0),
            Some(b'C') => EncodedValue::Char(0),
            Some(b'I') => EncodedValue::Int(0),
            Some(b'J') => EncodedValue::Long(0),
            Some(b'F') => EncodedValue::Float(0.0),
            Some(b'D') => EncodedValue::Double(0.0),
            _ => EncodedValue::Null,
        }
    }

    /// Builds the class-type index up front instead of on the first lookup.
    pub fn build_lookups(&self) {
        self.classes.index_of_type(TypeIdx(0));
    }

    pub(crate) fn ref_filter(&self) -> crate::error::Result<&RefFilter> {
        if let Some(filter) = self.ref_filter.get() {
            return Ok(filter);
        }
        let filter = RefFilter::build(self)?;
        self.release_pages();
        Ok(self.ref_filter.get_or_init(|| filter))
    }

    pub fn ref_filter_heap_bytes(&self) -> u64 {
        self.ref_filter.get().map_or(0, RefFilter::heap_bytes)
    }

    /// Materializes the class for mutation and marks the DEX dirty.
    /// Source reference filters remain valid because scans bypass them for resident classes.
    pub fn class_mut(&mut self, class_idx: usize) -> crate::error::Result<&mut ClassDef> {
        self.touch();
        self.classes.materialize(class_idx, self.parse_options)
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
        self.classes.materialize(class_idx, self.parse_options)?;
        Ok(true)
    }

    pub fn resolve_all_class_data(&mut self) -> crate::error::Result<()> {
        self.classes.materialize_all(self.parse_options)
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
