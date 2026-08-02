// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{require_len, Result};
use crate::types::instruction::Instruction;

use super::invoke::{decode_35c_invoke, decode_3rc_invoke, decode_invoke_polymorphic};

mod access;
mod basic;
mod ops;

pub(super) struct DecodedInstruction {
    instruction: Instruction,
    units: usize,
}

impl DecodedInstruction {
    pub(super) fn new(instruction: Instruction, units: usize) -> Self {
        Self { instruction, units }
    }
}

pub(super) fn nibbles(unit: u16) -> (u8, u8) {
    let a = ((unit >> 8) & 0xF) as u8;
    let b = ((unit >> 12) & 0xF) as u8;
    (a, b)
}

pub(super) fn hi8(unit: u16) -> u8 {
    (unit >> 8) as u8
}

fn min_instruction_bytes(opcode: u8) -> usize {
    match opcode {
        0x03
        | 0x06
        | 0x09
        | 0x14
        | 0x17
        | 0x1b
        | 0x24
        | 0x25
        | 0x26
        | 0x2a
        | 0x2b
        | 0x2c
        | 0x6e..=0x72
        | 0x74..=0x78
        | 0xfc
        | 0xfd => 6,
        0x18 => 10,
        0xfa | 0xfb => 8,
        0x00
        | 0x01
        | 0x04
        | 0x07
        | 0x0a..=0x12
        | 0x1d..=0x1e
        | 0x21
        | 0x27..=0x28
        | 0x3e..=0x43
        | 0x73
        | 0x79..=0x7a
        | 0x7b..=0x8f
        | 0xb0..=0xcf
        | 0xe3..=0xf9 => 2,
        _ => 4,
    }
}

pub fn decode_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::with_capacity(insns_size);
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
    let mut pc = 0usize;

    while pc < insns_size {
        let unit_off = start + pc * 2;
        require_len(buf, unit_off, 2, "code item instruction")?;
        let unit0 = super::format::u16_at(buf, unit_off);
        let opcode = (unit0 & 0xFF) as u8;
        require_len(
            buf,
            unit_off,
            min_instruction_bytes(opcode),
            "code item instruction",
        )?;

        let decoded = match opcode {
            0x00..=0x43 => basic::decode_opcode(buf, unit_off, opcode)?,
            0x44..=0x6d => access::decode_opcode(buf, unit_off, opcode)?,
            0x6e..=0x72 => DecodedInstruction::new(decode_35c_invoke(buf, unit_off, opcode), 3),
            0x73 => DecodedInstruction::new(Instruction::Nop, 1),
            0x74..=0x78 => DecodedInstruction::new(decode_3rc_invoke(buf, unit_off, opcode), 3),
            0x79..=0x7a => DecodedInstruction::new(Instruction::Nop, 1),
            0x7b..=0xe2 => ops::decode_opcode(buf, unit_off, opcode)?,
            0xe3..=0xf9 => DecodedInstruction::new(Instruction::Nop, 1),
            0xfa..=0xfd => {
                let units = match opcode {
                    0xfa | 0xfb => 4,
                    _ => 3,
                };
                DecodedInstruction::new(decode_invoke_polymorphic(buf, unit_off, opcode), units)
            }
            0xfe..=0xff => basic::decode_opcode(buf, unit_off, opcode)?,
        };

        pc += decoded.units;
        out.push(decoded.instruction);
    }

    Ok(())
}
