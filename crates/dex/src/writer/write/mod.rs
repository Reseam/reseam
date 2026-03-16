//! DEX writer entrypoint and shared writer state.

use crate::error::Result;
use crate::model::dex_file::DexFile;
use crate::model::encoded_value::EncodedValue;
use crate::model::map::*;
use crate::model::types::TypeIdx;
use std::collections::HashMap;

mod annotations;
mod classdata;
mod code;
mod finalize;
mod orchestration;
mod types;

pub(crate) fn methods_with_code(dex: &DexFile) -> Vec<&crate::model::class::EncodedMethod> {
    dex.classes
        .iter()
        .filter_map(|c| c.class_data.as_ref())
        .flat_map(|d| d.direct_methods.iter().chain(d.virtual_methods.iter()))
        .filter(|m| m.code.is_some())
        .collect()
}

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
/// # Examples
///
/// ```no_run
/// use stitch_dex::{parse, write, ParseOptions};
///
/// let bytes = std::fs::read("classes.dex")?;
/// let dex = parse(&bytes, ParseOptions::default())?;
/// let rewritten = write(&dex)?;
/// assert!(!rewritten.is_empty());
/// # Ok::<(), stitch_dex::DexError>(())
/// ```
pub fn write(dex: &DexFile) -> Result<Vec<u8>> {
    let sorted = super::sort::sort_for_write(dex);
    let mut w = DexWriter::new();
    w.write_dex(&sorted)?;
    Ok(w.buf)
}

/// Shared mutable state threaded through writer submodules.
pub(crate) struct DexWriter {
    pub(crate) buf: Vec<u8>,
    pub(crate) string_data_offsets: Vec<u32>,
    pub(crate) type_list_cache: HashMap<Vec<TypeIdx>, u32>,
    pub(crate) code_item_offsets: Vec<u32>,
    pub(crate) debug_info_offsets: Vec<u32>,
    pub(crate) class_data_offsets: Vec<u32>,
    pub(crate) debug_info_cache: HashMap<Vec<u8>, u32>,
    pub(crate) map_entries: Vec<MapItem>,
}

impl DexWriter {
    pub(crate) fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1024 * 1024),
            string_data_offsets: Vec::new(),
            type_list_cache: HashMap::new(),
            code_item_offsets: Vec::new(),
            debug_info_offsets: Vec::new(),
            class_data_offsets: Vec::new(),
            debug_info_cache: HashMap::new(),
            map_entries: Vec::new(),
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

pub(crate) use orchestration::DexWriterWriteExt;
