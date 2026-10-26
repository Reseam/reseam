// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, malformed};
use crate::instruction::Instruction;
use crate::instruction::opcodes::{
    AGET, CONST_METHOD_HANDLE, CONST_METHOD_TYPE, IF_LEZ, INVOKE_CUSTOM_RANGE, INVOKE_INTERFACE,
    INVOKE_INTERFACE_RANGE, INVOKE_POLYMORPHIC, INVOKE_VIRTUAL, INVOKE_VIRTUAL_RANGE, NEG_INT, NOP,
    SPUT_SHORT, USHR_INT_LIT8,
};
use crate::instruction::stream::instruction_stream;

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

pub fn decode_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::with_capacity(insns_size.min(buf.len().saturating_sub(start) / 2));
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
        instructions.push(decoded);
    }

    Ok(instructions)
}
