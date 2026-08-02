// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! DEX serialization entrypoints and helpers.

use self::orchestration::DexWriterWriteExt;
use crate::error::Result;
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::map::*;
use crate::types::TypeList;
use std::collections::HashMap;

pub(crate) mod annotations;
pub(crate) mod class_data;
pub(crate) mod code;
pub(crate) mod compact;
pub(crate) mod debug;
pub(crate) mod encoded_arrays;
pub(crate) mod encoded_value;
pub(crate) mod finalize;
pub(crate) mod instruction_writer;
pub(crate) mod orchestration;
pub(crate) mod sort;

/// Returns whether an encoded static value can be elided from the tail array.
pub(crate) fn is_default_value(v: &EncodedValue) -> bool {
    matches!(
        v,
        EncodedValue::Byte(0)
            | EncodedValue::Short(0)
            | EncodedValue::Char(0)
            | EncodedValue::Int(0)
            | EncodedValue::Long(0)
            | EncodedValue::Null
            | EncodedValue::Boolean(false)
    ) || matches!(v, EncodedValue::Float(f) if f.to_bits() == 0)
        || matches!(v, EncodedValue::Double(d) if d.to_bits() == 0)
}

/// Serializes a [`DexFile`] back into canonical DEX bytes.
///
/// Classes a patch never touched are never materialized: they are decoded,
/// remapped, and re-encoded one at a time straight from the original buffer,
/// so the writer's peak memory is bounded by the mutated classes plus one
/// in-flight class per worker rather than the whole DEX.
pub fn write(dex: &mut DexFile) -> Result<Vec<u8>> {
    validate_index_limits(dex)?;
    let remap = sort::sort_in_place(dex)?;
    let mut w = DexWriter::new();
    w.write_dex(dex, remap.as_ref())?;
    Ok(w.buf)
}

pub const MAX_POOL_SIZE: usize = 1 << 16;

fn validate_index_limits(dex: &DexFile) -> Result<()> {
    let checks: &[(&str, usize)] = &[
        ("type_ids", dex.types.len()),
        ("proto_ids", dex.prototypes.len()),
        ("field_ids", dex.fields.len()),
        ("method_ids", dex.methods.len()),
        ("call_site_ids", dex.call_sites.len()),
        ("method_handle_ids", dex.method_handles.len()),
    ];
    for &(name, count) in checks {
        if count > MAX_POOL_SIZE {
            return Err(crate::error::invalid(
                "dex",
                format!(
                    "{name} count {count} exceeds maximum {MAX_POOL_SIZE} — \
                     split into multiple DEX files"
                ),
            ));
        }
    }
    Ok(())
}

/// Serializes multiple [`DexFile`]s into a single v41 container buffer.
///
/// Each logical DEX file is written sequentially. All offsets are relative
/// to the start of the physical container.
pub fn write_container(dex_files: &mut [DexFile]) -> Result<Vec<u8>> {
    if dex_files.is_empty() {
        return Ok(Vec::new());
    }
    if dex_files.len() == 1 {
        return write(&mut dex_files[0]);
    }

    let mut w = DexWriter::new();

    let mut remaps = Vec::with_capacity(dex_files.len());
    for dex in dex_files.iter_mut() {
        validate_index_limits(dex)?;
        remaps.push(sort::sort_in_place(dex)?);
    }

    let total_count = dex_files.len();
    for (i, dex) in dex_files.iter().enumerate() {
        w.header_base = w.pos();
        w.container_size = 0;
        w.write_dex(dex, remaps[i].as_ref())?;

        if i + 1 < total_count {
            w.align(4);
        }
    }

    let container_size = w.pos();
    let mut offset = 0u32;
    for dex in dex_files.iter() {
        let header_size = dex.required_version().header_size();
        if header_size >= 0x78 {
            w.patch_u32(offset as usize + 0x70, container_size);
        }

        let start = offset as usize + 0x20;
        let bytes: [u8; 4] = w
            .buf
            .get(start..start + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| {
                crate::error::malformed("container", start, "file size field truncated")
            })?;
        let file_size_field = u32::from_le_bytes(bytes);
        offset += file_size_field;
        let padding = (4 - (offset % 4)) % 4;
        offset += padding;
    }

    Ok(w.buf)
}

/// Shared mutable state threaded through writer submodules.
pub(crate) struct DexWriter {
    pub(crate) buf: Vec<u8>,
    pub(crate) string_data_offsets: Vec<u32>,
    pub(crate) type_list_cache: HashMap<TypeList, u32>,
    pub(crate) code_item_offsets: Vec<u32>,
    pub(crate) class_data_offsets: Vec<u32>,
    pub(crate) map_entries: Vec<MapItem>,
    pub(crate) header_base: u32,
    pub(crate) container_size: u32,
}

impl DexWriter {
    pub(crate) fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1024 * 1024),
            string_data_offsets: Vec::new(),
            type_list_cache: HashMap::new(),
            code_item_offsets: Vec::new(),
            class_data_offsets: Vec::new(),
            map_entries: Vec::new(),
            header_base: 0,
            container_size: 0,
        }
    }

    pub(crate) fn pos(&self) -> u32 {
        self.buf.len() as u32
    }

    pub(crate) fn align(&mut self, alignment: usize) {
        let padding = (alignment - (self.buf.len() % alignment)) % alignment;
        self.buf.extend(std::iter::repeat_n(0u8, padding));
    }

    pub(crate) fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn patch_u32(&mut self, offset: usize, v: u32) {
        self.buf[offset..offset + 4].copy_from_slice(&v.to_le_bytes());
    }
}
