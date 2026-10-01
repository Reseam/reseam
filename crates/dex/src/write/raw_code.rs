// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::sort::Remap;
use crate::encoding::leb128::{write_sleb128, write_uleb128, write_uleb128p1};
use crate::error::{Result, require_len};
use crate::read::code::payload::{HandlerEvent, handler_index, walk_handler_list};
use crate::read::code::{index_operands, walk_instructions};
use crate::read::debug::walk_debug_info;
use crate::types::header::ParseOptions;

pub(crate) fn copy_code_item(
    buf: &[u8],
    code_off: u32,
    remap: Option<&Remap<'_>>,
    opts: ParseOptions,
    out: &mut Vec<u8>,
) -> Result<bool> {
    let base = code_off as usize;
    require_len(buf, base, 16, "code item")?;
    let tries_size = u16::from_le_bytes([buf[base + 6], buf[base + 7]]) as usize;
    let insns_size = crate::read::u32_at(buf, base + 12) as usize;
    let insns_start = base + 16;
    crate::error::require_array(buf, insns_start, insns_size, 2, "code item instructions")?;

    let start = out.len();
    out.extend_from_slice(&buf[base..insns_start + insns_size * 2]);
    out[start + 8..start + 12].fill(0);

    if let Some(remap) = remap {
        let widened =
            !remap_instruction_stream(buf, insns_start, insns_size, remap, &mut out[start + 16..])?;
        if widened {
            out.truncate(start);
            return Ok(false);
        }
    }

    if tries_size == 0 {
        return Ok(true);
    }
    let mut pos = insns_start + insns_size * 2;
    if !insns_size.is_multiple_of(2) {
        require_len(buf, pos, 2, "code item padding")?;
        out.extend_from_slice(&buf[pos..pos + 2]);
        pos += 2;
    }
    let tries_start = out.len();
    require_len(buf, pos, tries_size * 8, "try items")?;
    out.extend_from_slice(&buf[pos..pos + tries_size * 8]);
    let list_off = pos + tries_size * 8;
    let list_start = out.len();
    let mut old_offsets = Vec::new();
    let mut new_offsets = Vec::new();
    let end = walk_handler_list(buf, list_off, opts, |event| {
        match event {
            HandlerEvent::Count(count) => {
                if remap.is_some() {
                    write_uleb128(out, count);
                }
            }
            HandlerEvent::Handler { offset, size } => {
                old_offsets.push(offset);
                new_offsets.push(if remap.is_some() {
                    out.len() - list_start
                } else {
                    offset
                });
                if remap.is_some() {
                    write_sleb128(out, size);
                }
            }
            HandlerEvent::Typed(catch) => {
                if let Some(remap) = remap {
                    write_uleb128(
                        out,
                        remap.index(crate::types::Pool::Type, catch.exception_type.0)?,
                    );
                    write_uleb128(out, catch.addr);
                }
            }
            HandlerEvent::CatchAll(addr) => {
                if remap.is_some() {
                    write_uleb128(out, addr);
                }
            }
        }
        Ok(())
    })?;
    if remap.is_none() {
        out.extend_from_slice(&buf[list_off..end]);
    }
    for index in 0..tries_size {
        let at = tries_start + index * 8 + 6;
        let old = usize::from(u16::from_le_bytes([out[at], out[at + 1]]));
        let mapped = new_offsets[handler_index(&old_offsets, old)?];
        let mapped = u16::try_from(mapped).map_err(|_| {
            crate::error::invalid(
                "catch handler",
                "referenced handler offset exceeds 65535 bytes",
            )
        })?;
        out[at..at + 2].copy_from_slice(&mapped.to_le_bytes());
    }
    Ok(true)
}

fn remap_operands(opcode: u8, remap: &Remap<'_>, insn: &mut [u8]) -> Result<bool> {
    for operand in index_operands(opcode) {
        let at = operand.at;
        if matches!(operand.width, crate::read::code::refs::IndexWidth::U32) {
            let old = crate::read::u32_at(insn, at);
            insn[at..at + 4].copy_from_slice(&remap.index(operand.pool, old)?.to_le_bytes());
            continue;
        }
        let old = u32::from(u16::from_le_bytes([insn[at], insn[at + 1]]));
        let Ok(new) = u16::try_from(remap.index(operand.pool, old)?) else {
            return Ok(false);
        };
        insn[at..at + 2].copy_from_slice(&new.to_le_bytes());
    }
    Ok(true)
}

pub(crate) fn copy_debug_info(
    buf: &[u8],
    off: u32,
    remap: Option<&Remap<'_>>,
    opts: ParseOptions,
    out: &mut Vec<u8>,
) -> Result<()> {
    walk_debug_info(buf, off, opts, |record| {
        let mut start = record.range.start;
        if let Some(remap) = remap {
            for index in record.indices {
                out.extend_from_slice(&buf[start..index.range.start]);
                write_uleb128p1(
                    out,
                    index
                        .value
                        .map(|value| remap.index(index.pool, value))
                        .transpose()?,
                );
                start = index.range.end;
            }
        }
        out.extend_from_slice(&buf[start..record.range.end]);
        Ok(())
    })
}

fn remap_instruction_stream(
    buf: &[u8],
    insns_start: usize,
    insns_size: usize,
    remap: &Remap<'_>,
    out: &mut [u8],
) -> Result<bool> {
    let mut widened = false;
    let mut mapping_error = None;
    walk_instructions(buf, insns_start, insns_size, |insn| {
        let at = insn.offset() - insns_start;
        match remap_operands(insn.opcode, remap, &mut out[at..]) {
            Ok(fits) => {
                widened = !fits;
                fits
            }
            Err(error) => {
                mapping_error = Some(error);
                false
            }
        }
    })?;
    if let Some(error) = mapping_error {
        return Err(error);
    }
    Ok(!widened)
}
