// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use super::instruction_writer::encode_instructions;
use super::sort::RemapTables;
use super::DexWriter;
use crate::encoding::leb128::{write_sleb128, write_uleb128};
use crate::error::Result;
use crate::file::DexFile;
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, EncodedField};
use crate::types::code::CodeItem;
use crate::types::map::{MapItem, TYPE_CODE_ITEM, TYPE_DEBUG_INFO_ITEM};
use crate::types::MethodIdx;

/// A method's class-data entry with its code stripped: enough to emit the
/// `class_data_item` after the code items are laid out, without keeping any
/// decoded instructions resident.
pub(crate) struct MethodLayout {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub has_code: bool,
}

/// A class's `class_data_item` shape, retained while its (heavy) code is
/// dropped. Cheap: a few integers per member.
pub(crate) struct ClassLayout {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<MethodLayout>,
    pub virtual_methods: Vec<MethodLayout>,
}

/// A class's members prepared for writing: borrowed when the class is resident
/// (a patch touched it), owned when it was decoded on demand from the raw
/// buffer.
enum PreparedClassData<'a> {
    Borrowed(&'a ClassData),
    Owned(ClassData),
}

impl PreparedClassData<'_> {
    fn data(&self) -> &ClassData {
        match self {
            Self::Borrowed(data) => data,
            Self::Owned(data) => data,
        }
    }
}

/// Yields a class's fully-prepared `class_data` (indices remapped, members
/// sorted, instructions widened) without permanently materializing it.
///
/// Resident classes were already remapped and widened by [`super::sort`], so
/// they are borrowed as-is. A deferred class is decoded from the original
/// buffer, then put through the exact same remap + widening the resident path
/// applied, and returned owned so the caller drops it after emission.
fn prepare_class_data<'a>(
    dex: &'a DexFile,
    class_idx: usize,
    remap: Option<&RemapTables>,
) -> Result<Option<PreparedClassData<'a>>> {
    if let Some(data) = &dex.classes[class_idx].class_data {
        return Ok(Some(PreparedClassData::Borrowed(data)));
    }

    let Some(offset) = dex.raw_class_data_offset(class_idx) else {
        return Ok(None);
    };
    let buf = dex.raw_bytes(offset)?;
    let mut data = crate::read::class::read_class_data_at(buf, offset as usize, &dex.parse_options)?;

    if let Some(tables) = remap {
        tables.as_remap().remap_class_data(&mut data);
        for method in data
            .direct_methods
            .iter_mut()
            .chain(data.virtual_methods.iter_mut())
        {
            if let Some(code) = method.code.as_mut() {
                super::sort::fixup_code(code)?;
            }
        }
    }

    Ok(Some(PreparedClassData::Owned(data)))
}

/// Writes all code items and their deduplicated debug info in a single pass,
/// streaming deferred classes one at a time, and returns the per-class layout
/// the class-data section needs afterwards.
///
/// Layout matches the eager writer byte-for-byte: every `code_item` first (in
/// class-then-direct-then-virtual order), then the contiguous debug-info
/// section, then the code items' `debug_info_off` fields backpatched.
pub(crate) fn write_code_and_debug(
    w: &mut DexWriter,
    dex: &DexFile,
    remap: Option<&RemapTables>,
) -> Result<Vec<Option<ClassLayout>>> {
    let code_start = w.pos();
    w.code_item_offsets.clear();

    let mut debug_buf: Vec<u8> = Vec::new();
    let mut debug_cache: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut code_debug_local: Vec<Option<u32>> = Vec::new();
    let mut layouts: Vec<Option<ClassLayout>> = Vec::with_capacity(dex.classes.len());

    for class_idx in 0..dex.classes.len() {
        let Some(prepared) = prepare_class_data(dex, class_idx, remap)? else {
            layouts.push(None);
            continue;
        };
        let data = prepared.data();

        let mut layout = ClassLayout {
            static_fields: data.static_fields.clone(),
            instance_fields: data.instance_fields.clone(),
            direct_methods: Vec::with_capacity(data.direct_methods.len()),
            virtual_methods: Vec::with_capacity(data.virtual_methods.len()),
        };

        for (is_virtual, methods) in [
            (false, &data.direct_methods),
            (true, &data.virtual_methods),
        ] {
            for method in methods {
                if let Some(code) = &method.code {
                    w.align(4);
                    let off = w.pos();
                    w.code_item_offsets.push(off);
                    write_code_item(w, code)?;
                    code_debug_local.push(debug_local_offset(
                        code,
                        &mut debug_buf,
                        &mut debug_cache,
                    ));
                }
                let entry = MethodLayout {
                    method: method.method,
                    access_flags: method.access_flags,
                    has_code: method.code.is_some(),
                };
                if is_virtual {
                    layout.virtual_methods.push(entry);
                } else {
                    layout.direct_methods.push(entry);
                }
            }
        }

        layouts.push(Some(layout));
    }

    let code_item_count = w.code_item_offsets.len() as u32;
    if code_item_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_CODE_ITEM,
            size: code_item_count,
            offset: code_start,
        });
    }

    let debug_start = w.pos();
    if !debug_buf.is_empty() {
        w.buf.extend_from_slice(&debug_buf);
        w.map_entries.push(MapItem {
            type_code: TYPE_DEBUG_INFO_ITEM,
            size: debug_cache.len() as u32,
            offset: debug_start,
        });
    }

    for (ci_idx, local) in code_debug_local.iter().enumerate() {
        let ci_off = w.code_item_offsets[ci_idx] as usize;
        let value = local.map_or(0, |l| debug_start + l);
        w.patch_u32(ci_off + 8, value);
    }

    Ok(layouts)
}

/// Encodes a method's debug info into the shared debug buffer, deduplicating by
/// encoded bytes, and returns its offset within that buffer (or `None` when the
/// method has no debug info).
fn debug_local_offset(
    code: &CodeItem,
    debug_buf: &mut Vec<u8>,
    debug_cache: &mut HashMap<Vec<u8>, u32>,
) -> Option<u32> {
    let debug = code.debug_info.as_ref()?;
    let mut tmp = Vec::new();
    super::debug::write_debug_info(&mut tmp, debug);
    Some(match debug_cache.get(&tmp) {
        Some(&off) => off,
        None => {
            let off = debug_buf.len() as u32;
            debug_buf.extend_from_slice(&tmp);
            debug_cache.insert(tmp, off);
            off
        }
    })
}

/// Writes one `code_item`, including the shared encoded catch-handler stream.
fn write_code_item(w: &mut DexWriter, code: &CodeItem) -> Result<()> {
    w.write_u16(code.registers_size);
    w.write_u16(code.ins_size);
    w.write_u16(code.compute_outs_size());
    w.write_u16(code.tries.len() as u16);
    w.write_u32(0);
    let insns = encode_instructions(&code.instructions)?;
    w.write_u32(insns.len() as u32);
    for unit in &insns {
        w.write_u16(*unit);
    }

    if !code.tries.is_empty() {
        if !insns.len().is_multiple_of(2) {
            w.write_u16(0);
        }

        let mut handler_buf: Vec<u8> = Vec::new();
        write_uleb128(&mut handler_buf, code.catch_handlers.len() as u32);
        let mut handler_byte_offsets: Vec<u16> = Vec::new();
        for handler in &code.catch_handlers {
            handler_byte_offsets.push(handler_buf.len() as u16);
            let size = if handler.catch_all_addr.is_some() {
                -(handler.typed_catches.len() as i32)
            } else {
                handler.typed_catches.len() as i32
            };
            write_sleb128(&mut handler_buf, size);
            for tc in &handler.typed_catches {
                write_uleb128(&mut handler_buf, tc.exception_type.0);
                write_uleb128(&mut handler_buf, tc.addr);
            }
            if let Some(addr) = handler.catch_all_addr {
                write_uleb128(&mut handler_buf, addr);
            }
        }

        for t in &code.tries {
            w.write_u32(t.start_addr);
            w.write_u16(t.insn_count);
            w.write_u16(handler_byte_offsets[t.handler_idx]);
        }

        w.buf.extend_from_slice(&handler_buf);
    }
    Ok(())
}
