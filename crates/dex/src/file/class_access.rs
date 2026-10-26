// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::DexFile;
use super::ref_index::RefIndex;
use crate::error::invalid_offset;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::types::FieldIdx;
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;

impl DexFile {
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

    pub(crate) fn ref_index(&self) -> crate::error::Result<&RefIndex> {
        if let Some(index) = self.ref_index.get() {
            return Ok(index);
        }
        let index = RefIndex::build(self)?;
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

    /// Materializes the class for mutation and marks the DEX dirty.
    /// Source reference filters remain valid because scans bypass them for resident classes.
    pub fn class_mut(&mut self, class_idx: usize) -> crate::error::Result<&mut ClassDef> {
        self.touch();
        self.classes.materialize(class_idx, self.parse_options)
    }
}
