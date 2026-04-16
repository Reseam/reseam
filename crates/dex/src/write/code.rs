// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction_writer::encode_instructions;
use super::DexWriter;
use crate::encoding::leb128::{write_sleb128, write_uleb128};
use crate::error::Result;

/// Writes code items, deduplicated debug info, and code-item backpatches.
pub(crate) fn write_code_items(
    w: &mut DexWriter,
    methods: &[(
        &crate::types::class::EncodedMethod,
        &crate::types::code::CodeItem,
    )],
) -> Result<()> {
    let code_start = w.pos();
    w.code_item_offsets.clear();
    for (_, code) in methods {
        w.align(4);
        let off = w.pos();
        w.code_item_offsets.push(off);
        write_code_item(w, code)?;
    }
    let code_item_count = methods.len() as u32;
    if code_item_count > 0 {
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_CODE_ITEM,
            size: code_item_count,
            offset: code_start,
        });
    }

    let debug_start = w.pos();
    let mut debug_count = 0u32;
    w.debug_info_offsets.clear();
    for (_, code) in methods {
        if let Some(ref debug) = code.debug_info {
            let mut tmp = Vec::new();
            super::debug::write_debug_info(&mut tmp, debug);
            if let Some(&cached_off) = w.debug_info_cache.get(&tmp) {
                w.debug_info_offsets.push(cached_off);
            } else {
                let off = w.pos();
                w.debug_info_offsets.push(off);
                w.buf.extend_from_slice(&tmp);
                w.debug_info_cache.insert(tmp, off);
                debug_count += 1;
            }
        } else {
            w.debug_info_offsets.push(0);
        }
    }
    if debug_count > 0 {
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_DEBUG_INFO_ITEM,
            size: debug_count,
            offset: debug_start,
        });
    }

    {
        let mut di_idx = 0;
        for (ci_idx, (_, code)) in methods.iter().enumerate() {
            let ci_off = w.code_item_offsets[ci_idx] as usize;
            if code.debug_info.is_some() {
                w.patch_u32(ci_off + 8, w.debug_info_offsets[di_idx]);
                di_idx += 1;
            } else {
                w.patch_u32(ci_off + 8, 0);
            }
        }
    }
    Ok(())
}

/// Writes one `code_item`, including the shared encoded catch-handler stream.
fn write_code_item(w: &mut DexWriter, code: &crate::types::code::CodeItem) -> Result<()> {
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
