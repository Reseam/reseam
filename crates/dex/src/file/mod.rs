// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod class_ops;
mod classes;
pub(crate) mod hidden_api;
pub(crate) mod memory;
pub use hidden_api::HiddenApiData;
mod ids;
mod interning;
mod items;
pub(crate) use items::FileTable;
mod lookup;
mod pools;
mod sorted_index;
mod static_values;
mod strings;
#[cfg(test)]
mod tests;
mod type_lookups;
mod undo;
mod version;

use crate::DexBytes;
use crate::search::index::{PoolKeys, RefIndex};
pub use classes::ClassTable;
pub use ids::{IdRecord, IdTable};
pub use strings::StringPool;

use crate::model::header::DexHeader;
use crate::model::method_handle::{CallSiteItem, MethodHandle};
use crate::model::{FieldId, MethodId, Prototype, StringIdx};
use crate::options::ParseOptions;

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
    ref_index: std::sync::OnceLock<RefIndex>,
    type_lookups: type_lookups::TypeLookups,
    dirty: bool,
    write_options: crate::options::WriteOptions,
    journal: reseam_journal::Journal<undo::DexPart, undo::DexUndo>,
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
            ref_index: std::sync::OnceLock::new(),
            type_lookups: type_lookups::TypeLookups::default(),
            dirty: false,
            write_options: crate::options::WriteOptions::default(),
            journal: reseam_journal::Journal::default(),
        }
    }

    pub fn header(&self) -> &DexHeader {
        &self.header
    }

    pub fn raw_buffer(&self) -> Option<&[u8]> {
        self.raw.as_ref().map(DexBytes::as_bytes)
    }

    pub fn release_pages(&self) {
        if let Some(raw) = &self.raw {
            raw.release_pages();
        }
    }

    pub fn write_options(&self) -> crate::options::WriteOptions {
        self.write_options
    }

    /// Changes serialization policy and marks the DEX dirty when it changes.
    pub fn set_write_options(&mut self, options: crate::options::WriteOptions) {
        if self.write_options != options {
            self.touch();
            self.journal.record(undo::DexPart::WriteOptions, || {
                undo::DexUndo::WriteOptions(self.write_options)
            });
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

    /// Whether anything changed since parse or the last [`Self::mark_clean`],
    /// so the writer can copy untouched files through verbatim.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Every change starts here, so the first one under a savepoint keeps the
    /// lengths of the append-only pools as they were before it.
    pub(crate) fn touch(&mut self) {
        let lens = self.pool_lens();
        self.journal
            .record(undo::DexPart::Pools, || undo::DexUndo::Pools(lens));
        if !self.dirty {
            tracing::debug!(file_size = self.header.file_size, "dex marked dirty");
        }
        self.dirty = true;
    }

    pub(crate) fn ref_index(&self) -> crate::error::Result<&RefIndex> {
        if let Some(index) = self.ref_index.get() {
            return Ok(index);
        }
        let keys = PoolKeys::new(self.methods.len(), self.fields.len(), self.strings.len());
        let index = RefIndex::build(&self.classes, keys, self.parse_options)?;
        self.release_pages();
        Ok(self.ref_index.get_or_init(|| index))
    }

    /// Drops the search caches: the reference index, the cached references of
    /// resident classes and remembered type lookups. The next search builds
    /// them again.
    pub fn release_search_caches(&mut self) {
        self.ref_index.take();
        self.classes.release_refs();
        self.type_lookups.clear();
    }

    pub fn ref_index_heap_bytes(&self) -> u64 {
        self.ref_index.get().map_or(0, RefIndex::heap_bytes)
    }
}
