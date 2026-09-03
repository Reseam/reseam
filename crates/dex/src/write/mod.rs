// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! DEX serialization entrypoints and helpers.

use self::orchestration::DexWriterWriteExt;
pub use self::sink::{DexSink, SpoolSink, Spooled};
use crate::error::Result;
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::map::*;

pub(crate) mod annotations;
pub(crate) mod class_data;
pub(crate) mod code;
pub(crate) mod compact;
pub(crate) mod debug;
pub(crate) mod encoded_arrays;
pub(crate) mod encoded_value;
pub(crate) mod finalize;
pub(crate) mod instruction_writer;
pub(crate) mod intern;
pub(crate) mod orchestration;
pub(crate) mod plan;
pub(crate) mod raw_code;
pub(crate) mod sink;
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
/// in-flight class per worker rather than the whole DEX. The file itself is
/// not modified.
pub fn write(dex: &DexFile) -> Result<Vec<u8>> {
    write_into(dex, Vec::new())
}

/// Serializes into an anonymous temp file instead of memory.
pub fn write_spooled(dex: &DexFile) -> Result<Spooled> {
    write_into(dex, SpoolSink::new().map_err(crate::error::DexError::Io)?)?.finish()
}

fn write_into<S: DexSink>(dex: &DexFile, sink: S) -> Result<S> {
    validate_index_limits(dex)?;
    let plan = plan::WritePlan::new(dex)?;
    let mut w = DexWriter::new(sink);
    w.write_dex(&plan)?;
    Ok(w.sink)
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
pub fn write_container(dex_files: &[DexFile]) -> Result<Vec<u8>> {
    if dex_files.is_empty() {
        return Ok(Vec::new());
    }
    if dex_files.len() == 1 {
        return write(&dex_files[0]);
    }

    let mut w = DexWriter::new(Vec::new());

    let total_count = dex_files.len();
    let mut file_sizes = Vec::with_capacity(total_count);
    for (i, dex) in dex_files.iter().enumerate() {
        validate_index_limits(dex)?;
        let plan = plan::WritePlan::new(dex)?;
        w.header_base = w.pos();
        w.container_size = 0;
        w.write_dex(&plan)?;
        file_sizes.push(w.pos() - w.header_base);

        if i + 1 < total_count {
            w.align(4);
        }
    }

    let container_size = w.pos();
    let mut offset = 0u32;
    for (dex, &file_size) in dex_files.iter().zip(&file_sizes) {
        let header_size = dex.required_version().header_size();
        if header_size >= 0x78 {
            w.patch_u32(offset as usize + 0x70, container_size);
        }
        offset += file_size;
        offset += (4 - (offset % 4)) % 4;
    }

    Ok(w.sink)
}

/// Shared mutable state threaded through writer submodules.
pub(crate) struct DexWriter<S: DexSink> {
    pub(crate) sink: S,
    pub(crate) string_data_offsets: Vec<u32>,
    pub(crate) code_item_offsets: Vec<u32>,
    pub(crate) class_data_offsets: Vec<u32>,
    pub(crate) map_entries: Vec<MapItem>,
    pub(crate) header_base: u32,
    pub(crate) container_size: u32,
}

impl<S: DexSink> DexWriter<S> {
    pub(crate) fn new(sink: S) -> Self {
        Self {
            sink,
            string_data_offsets: Vec::new(),
            code_item_offsets: Vec::new(),
            class_data_offsets: Vec::new(),
            map_entries: Vec::new(),
            header_base: 0,
            container_size: 0,
        }
    }

    pub(crate) fn pos(&self) -> u32 {
        self.sink.pos()
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        self.sink.write(bytes);
    }

    pub(crate) fn write_zeros(&mut self, count: usize) {
        self.sink.write(&vec![0u8; count]);
    }

    pub(crate) fn align(&mut self, alignment: usize) {
        let padding = (alignment - (self.pos() as usize % alignment)) % alignment;
        self.write_zeros(padding);
    }

    pub(crate) fn write_u16(&mut self, v: u16) {
        self.sink.write(&v.to_le_bytes());
    }
    pub(crate) fn write_u32(&mut self, v: u32) {
        self.sink.write(&v.to_le_bytes());
    }

    pub(crate) fn write_uleb128(&mut self, v: u32) {
        let (bytes, len) = crate::encoding::leb128::encode_uleb128(v);
        self.sink.write(&bytes[..len]);
    }

    pub(crate) fn patch(&mut self, offset: usize, bytes: &[u8]) {
        self.sink.patch(offset, bytes);
    }

    pub(crate) fn patch_u32(&mut self, offset: usize, v: u32) {
        self.sink.patch(offset, &v.to_le_bytes());
    }
}
