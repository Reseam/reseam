// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Copies code and debug items of file classes with their pool indices
//! rewritten in place, so a method a patch never touched is never decoded.

use super::sort::Remap;
use crate::encoding::leb128::{
    read_sleb128_with_opts, read_uleb128_with_opts, read_uleb128p1_with_opts, write_sleb128,
    write_uleb128, write_uleb128p1,
};
use crate::error::{require_len, Result};
use crate::read::code::walk_instructions;
use crate::types::header::ParseOptions;
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

/// Appends the code item at `code_off` to `out` with every pool index
/// remapped and `debug_info_off` cleared for later patching. Returns `false`
/// without writing when a `const-string` operand outgrows its 16-bit form,
/// which needs the decoding path to widen it.
pub(crate) fn copy_code_item(
    buf: &[u8],
    code_off: u32,
    remap: Option<&Remap<'_>>,
    opts: &ParseOptions,
    out: &mut Vec<u8>,
) -> Result<bool> {
    let base = code_off as usize;
    require_len(buf, base, 16, "code item")?;
    let tries_size = u16::from_le_bytes([buf[base + 6], buf[base + 7]]) as usize;
    let insns_size = u32::from_le_bytes(buf[base + 12..base + 16].try_into().unwrap()) as usize;
    let insns_start = base + 16;
    require_len(buf, insns_start, insns_size * 2, "code item instructions")?;

    let start = out.len();
    out.extend_from_slice(&buf[base..insns_start + insns_size * 2]);
    out[start + 8..start + 12].fill(0);

    if let Some(remap) = remap {
        let mut widened = false;
        walk_instructions(buf, insns_start, insns_size, |insn| {
            let at = start + 16 + (insn.offset() - insns_start);
            widened = !remap_operands(insn.opcode, remap, &mut out[at..]);
            !widened
        })?;
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
    match remap {
        None => {
            let end = handler_list_end(buf, list_off, opts)?;
            out.extend_from_slice(&buf[list_off..end]);
        }
        Some(remap) => {
            let moved = copy_handlers(buf, list_off, remap, opts, out)?;
            for i in 0..tries_size {
                let at = tries_start + i * 8 + 6;
                let old = u16::from_le_bytes([out[at], out[at + 1]]);
                if let Some(&(_, new)) = moved.iter().find(|&&(o, _)| o == old) {
                    out[at..at + 2].copy_from_slice(&new.to_le_bytes());
                }
            }
        }
    }
    Ok(true)
}

/// Rewrites the pool index carried by the instruction at `insn[0..]`, the
/// same operands [`Remap::remap_instruction`] touches. Returns `false` when
/// a `const-string` index no longer fits.
fn remap_operands(opcode: u8, remap: &Remap<'_>, insn: &mut [u8]) -> bool {
    let get16 = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]) as u32;
    let set16 = |b: &mut [u8], at: usize, v: u32| b[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
    match opcode {
        0x1a => {
            let new = remap.remap_string(StringIdx(get16(insn, 2))).0;
            if new > 0xFFFF {
                return false;
            }
            set16(insn, 2, new);
        }
        0x1b => {
            let old = u32::from_le_bytes(insn[2..6].try_into().unwrap());
            let new = remap.remap_string(StringIdx(old)).0;
            insn[2..6].copy_from_slice(&new.to_le_bytes());
        }
        0x1c | 0x1f | 0x20 | 0x22..=0x25 => {
            set16(insn, 2, remap.remap_type(TypeIdx(get16(insn, 2))).0);
        }
        0x52..=0x6d => set16(insn, 2, remap.remap_field(FieldIdx(get16(insn, 2))).0),
        0x6e..=0x72 | 0x74..=0x78 => {
            set16(insn, 2, remap.remap_method(MethodIdx(get16(insn, 2))).0);
        }
        0xfa | 0xfb => {
            set16(insn, 2, remap.remap_method(MethodIdx(get16(insn, 2))).0);
            set16(insn, 6, remap.remap_proto(ProtoIdx(get16(insn, 6) as u16)).0 as u32);
        }
        0xff => set16(insn, 2, remap.remap_proto(ProtoIdx(get16(insn, 2) as u16)).0 as u32),
        _ => {}
    }
    true
}

/// Byte offset just past the `encoded_catch_handler_list` at `list_off`.
fn handler_list_end(buf: &[u8], list_off: usize, opts: &ParseOptions) -> Result<usize> {
    let (count, n) = read_uleb128_with_opts(buf, list_off, opts)?;
    let mut pos = list_off + n;
    for _ in 0..count {
        let (size, n) = read_sleb128_with_opts(buf, pos, opts)?;
        pos += n;
        for _ in 0..size.unsigned_abs() * 2 + u32::from(size <= 0) {
            pos += read_uleb128_with_opts(buf, pos, opts)?.1;
        }
    }
    Ok(pos)
}

/// Re-encodes the handler list with catch types remapped and returns each
/// handler's `(old, new)` byte offset within the list.
fn copy_handlers(
    buf: &[u8],
    list_off: usize,
    remap: &Remap<'_>,
    opts: &ParseOptions,
    out: &mut Vec<u8>,
) -> Result<Vec<(u16, u16)>> {
    let list_start = out.len();
    let (count, n) = read_uleb128_with_opts(buf, list_off, opts)?;
    let mut pos = list_off + n;
    write_uleb128(out, count);
    let mut moved = Vec::with_capacity(count as usize);
    for _ in 0..count {
        moved.push(((pos - list_off) as u16, (out.len() - list_start) as u16));
        let (size, n) = read_sleb128_with_opts(buf, pos, opts)?;
        pos += n;
        write_sleb128(out, size);
        for _ in 0..size.unsigned_abs() {
            let (type_idx, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;
            let (addr, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;
            write_uleb128(out, remap.remap_type(TypeIdx(type_idx)).0);
            write_uleb128(out, addr);
        }
        if size <= 0 {
            let (addr, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;
            write_uleb128(out, addr);
        }
    }
    Ok(moved)
}

/// Appends the debug info item at `off` to `out` with string and type
/// indices remapped; a verbatim copy when nothing moved.
pub(crate) fn copy_debug_info(
    buf: &[u8],
    off: u32,
    remap: Option<&Remap<'_>>,
    opts: &ParseOptions,
    out: &mut Vec<u8>,
) -> Result<()> {
    let start = off as usize;
    let mut pos = start;
    let copy = |out: &mut Vec<u8>, from: usize, to: usize| out.extend_from_slice(&buf[from..to]);
    let string = |out: &mut Vec<u8>, pos: &mut usize| -> Result<()> {
        let (idx, n) = read_uleb128p1_with_opts(buf, *pos, opts)?;
        match remap {
            Some(remap) => {
                write_uleb128p1(out, idx.map(|i| remap.remap_string(StringIdx(i)).0));
            }
            None => copy(out, *pos, *pos + n),
        }
        *pos += n;
        Ok(())
    };
    let uleb = |out: &mut Vec<u8>, pos: &mut usize| -> Result<()> {
        let n = read_uleb128_with_opts(buf, *pos, opts)?.1;
        copy(out, *pos, *pos + n);
        *pos += n;
        Ok(())
    };

    uleb(out, &mut pos)?;
    let (params, n) = read_uleb128_with_opts(buf, pos, opts)?;
    copy(out, pos, pos + n);
    pos += n;
    for _ in 0..params {
        string(out, &mut pos)?;
    }
    loop {
        require_len(buf, pos, 1, "debug info")?;
        let opcode = buf[pos];
        out.push(opcode);
        pos += 1;
        match opcode {
            0x00 => return Ok(()),
            0x01 | 0x05 | 0x06 => uleb(out, &mut pos)?,
            0x02 => {
                let n = read_sleb128_with_opts(buf, pos, opts)?.1;
                copy(out, pos, pos + n);
                pos += n;
            }
            0x03 | 0x04 => {
                uleb(out, &mut pos)?;
                string(out, &mut pos)?;
                let (type_idx, n) = read_uleb128p1_with_opts(buf, pos, opts)?;
                match remap {
                    Some(remap) => {
                        write_uleb128p1(out, type_idx.map(|t| remap.remap_type(TypeIdx(t)).0));
                    }
                    None => copy(out, pos, pos + n),
                }
                pos += n;
                if opcode == 0x04 {
                    string(out, &mut pos)?;
                }
            }
            0x09 => string(out, &mut pos)?,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::code::read_code_item;
    use crate::read::debug::read_debug_info;
    use crate::types::code::{CatchHandler, CodeItem, TryItem, TypedCatch};
    use crate::types::debug::{DebugBytecode, DebugInfo};
    use crate::types::instruction::{Instruction, RegList};
    use crate::write::DexWriter;

    fn shifted(len: usize, by: u32) -> Vec<u32> {
        (0..len as u32).map(|i| i + by).collect()
    }

    #[test]
    fn raw_copy_matches_decode_remap_encode() {
        let code = CodeItem {
            registers_size: 4,
            ins_size: 1,
            outs_size: 1,
            debug_info: None,
            instructions: vec![
                Instruction::ConstString { dest: 0, string: StringIdx(3) },
                Instruction::CheckCast { ref_: 0, type_: TypeIdx(2) },
                Instruction::Sget { dest: 1, field: FieldIdx(5) },
                Instruction::InvokeStatic { method: MethodIdx(7), args: RegList::new() },
                Instruction::ReturnVoid,
            ],
            tries: vec![TryItem { start_addr: 0, insn_count: 4, handler_idx: 0 }],
            catch_handlers: vec![CatchHandler {
                typed_catches: vec![TypedCatch { exception_type: TypeIdx(120), addr: 4 }],
                catch_all_addr: Some(4),
            }],
        };
        let mut original = DexWriter::new(Vec::new());
        crate::write::code::write_code_item(&mut original, &code).unwrap();
        let buf = original.sink;

        let string = shifted(10, 1);
        let type_ = shifted(200, 10);
        let field = shifted(10, 2);
        let method = shifted(10, 3);
        let proto = shifted(10, 0);
        let remap = Remap {
            string: &string,
            type_: &type_,
            proto: &proto,
            field: &field,
            method: &method,
        };

        let mut raw = Vec::new();
        assert!(copy_code_item(&buf, 0, Some(&remap), &ParseOptions::default(), &mut raw).unwrap());

        let mut expected = read_code_item(&buf, 0, &ParseOptions::default()).unwrap();
        remap.remap_code(&mut expected);
        let mut encoded = DexWriter::new(Vec::new());
        crate::write::code::write_code_item(&mut encoded, &expected).unwrap();
        assert_eq!(raw, encoded.sink);

        let decoded = read_code_item(&raw, 0, &ParseOptions::default()).unwrap();
        assert_eq!(decoded.instructions, expected.instructions);
        assert_eq!(decoded.catch_handlers[0].typed_catches[0].exception_type, TypeIdx(130));
    }

    #[test]
    fn debug_copy_matches_decode_remap_encode() {
        let info = DebugInfo {
            line_start: 12,
            parameter_names: vec![Some(StringIdx(1)), None],
            bytecodes: vec![
                DebugBytecode::SetPrologueEnd,
                DebugBytecode::StartLocal { register: 2, name: Some(StringIdx(200)), type_: Some(TypeIdx(3)) },
                DebugBytecode::StartLocalExtended {
                    register: 3,
                    name: None,
                    type_: Some(TypeIdx(4)),
                    signature: Some(StringIdx(5)),
                },
                DebugBytecode::AdvancePc { advance: 3 },
                DebugBytecode::AdvanceLine { advance: -2 },
                DebugBytecode::SpecialAdvance { line_advance: 1, pc_advance: 2 },
                DebugBytecode::EndLocal { register: 2 },
                DebugBytecode::SetFile { name: Some(StringIdx(6)) },
                DebugBytecode::EndSequence,
            ],
        };
        let mut buf = vec![0xAA];
        crate::write::debug::write_debug_info(&mut buf, &info);
        buf.push(0xBB);

        let string = shifted(300, 100);
        let type_ = shifted(10, 1);
        let remap = Remap {
            string: &string,
            type_: &type_,
            proto: &[],
            field: &[],
            method: &[],
        };
        let mut raw = Vec::new();
        copy_debug_info(&buf, 1, Some(&remap), &ParseOptions::default(), &mut raw).unwrap();

        let mut expected = read_debug_info(&buf, 1, &ParseOptions::default()).unwrap();
        remap.remap_debug(&mut expected);
        let mut encoded = Vec::new();
        crate::write::debug::write_debug_info(&mut encoded, &expected);
        assert_eq!(raw, encoded);

        let mut verbatim = Vec::new();
        copy_debug_info(&buf, 1, None, &ParseOptions::default(), &mut verbatim).unwrap();
        assert_eq!(verbatim, buf[1..buf.len() - 1]);
    }
}
