// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::refs::instruction_stream;
use crate::error::{Result, malformed, require_len};
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    AGET, CONST_METHOD_HANDLE, CONST_METHOD_TYPE, FILL_ARRAY_DATA_PAYLOAD, IF_LEZ,
    INVOKE_CUSTOM_RANGE, INVOKE_INTERFACE, INVOKE_INTERFACE_RANGE, INVOKE_POLYMORPHIC,
    INVOKE_VIRTUAL, INVOKE_VIRTUAL_RANGE, NEG_INT, NOP, PACKED_SWITCH_PAYLOAD,
    SPARSE_SWITCH_PAYLOAD, SPUT_SHORT, USHR_INT_LIT8,
};

use super::invoke::{decode_3rc_invoke, decode_35c_invoke, decode_invoke_polymorphic};

mod access;
mod basic;
mod ops;

pub(super) fn nibbles(unit: u16) -> (u8, u8) {
    let a = ((unit >> 8) & 0xF) as u8;
    let b = ((unit >> 12) & 0xF) as u8;
    (a, b)
}

pub(super) fn hi8(unit: u16) -> u8 {
    (unit >> 8) as u8
}

pub(super) use crate::types::instruction_encoding::opcode_units;

/// Counts the instructions in a code item by walking opcode lengths, without
/// decoding operands or building instructions.
pub fn count_instructions(buf: &[u8], start: usize, insns_size: usize) -> Result<u32> {
    instruction_stream(buf, start, insns_size)?.try_fold(0, |count, frame| frame.map(|_| count + 1))
}

pub(super) fn payload_units(buf: &[u8], unit_off: usize, unit0: u16) -> Result<usize> {
    use crate::read::{u16_at, u32_at};
    Ok(match unit0 {
        PACKED_SWITCH_PAYLOAD => {
            require_len(buf, unit_off, 4, "packed-switch payload")?;
            4 + u16_at(buf, unit_off + 2) as usize * 2
        }
        SPARSE_SWITCH_PAYLOAD => {
            require_len(buf, unit_off, 4, "sparse-switch payload")?;
            2 + u16_at(buf, unit_off + 2) as usize * 4
        }
        FILL_ARRAY_DATA_PAYLOAD => {
            require_len(buf, unit_off, 8, "fill-array-data payload")?;
            let data_bytes = (u32_at(buf, unit_off + 4) as usize)
                .checked_mul(u16_at(buf, unit_off + 2) as usize)
                .and_then(|bytes| bytes.checked_add(8))
                .ok_or_else(|| {
                    malformed("fill-array-data payload", unit_off, "payload size overflow")
                })?;
            data_bytes.div_ceil(2)
        }
        _ => 1,
    })
}

pub fn decode_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::with_capacity(insns_size.min(buf.len().saturating_sub(start) / 2));
    decode_instructions_into(buf, start, insns_size, &mut instructions)?;
    Ok(instructions)
}

/// Decodes instructions into `out`, reusing its existing capacity.
///
/// `out` is cleared first. Scanning callers pass the same buffer for every
/// method so decoding allocates only when a method exceeds all previous sizes.
pub fn decode_instructions_into(
    buf: &[u8],
    start: usize,
    insns_size: usize,
    out: &mut Vec<Instruction>,
) -> Result<()> {
    out.clear();
    for frame in instruction_stream(buf, start, insns_size)? {
        let frame = frame?;
        let unit_off = frame.offset();
        let opcode = frame.opcode;
        let raw = || Instruction::Raw {
            code_units: buf[unit_off..unit_off + frame.units * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                .collect(),
        };
        let decoded = match u16::from(opcode) {
            0x3e..=0x43 => raw(),
            NOP if crate::read::u16_at(buf, unit_off) != 0
                && !matches!(crate::read::u16_at(buf, unit_off), 0x0100 | 0x0200 | 0x0300) =>
            {
                raw()
            }
            NOP..=IF_LEZ => basic::decode_opcode(buf, unit_off, opcode)?,
            AGET..=SPUT_SHORT => access::decode_opcode(buf, unit_off, opcode)?,
            INVOKE_VIRTUAL..=INVOKE_INTERFACE => decode_35c_invoke(buf, unit_off, opcode)?,
            0x73 => raw(),
            INVOKE_VIRTUAL_RANGE..=INVOKE_INTERFACE_RANGE => {
                decode_3rc_invoke(buf, unit_off, opcode)?
            }
            0x79..=0x7a => raw(),
            NEG_INT..=USHR_INT_LIT8 => ops::decode_opcode(buf, unit_off, opcode)?,
            0xe3..=0xf9 => raw(),
            INVOKE_POLYMORPHIC..=INVOKE_CUSTOM_RANGE => {
                decode_invoke_polymorphic(buf, unit_off, opcode)?
            }
            CONST_METHOD_HANDLE..=CONST_METHOD_TYPE => basic::decode_opcode(buf, unit_off, opcode)?,
            _ => {
                return Err(malformed(
                    "instruction opcode",
                    unit_off,
                    "opcode is outside the decoded families",
                ));
            }
        };

        debug_assert_eq!(frame.units, decoded.code_units() as usize);
        out.push(decoded);
    }

    Ok(())
}
