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

/// Code units of a fixed-format instruction; `0x00` may instead start a
/// variable-length payload, see [`payload_units`].
pub(super) fn opcode_units(opcode: u8) -> usize {
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
        | 0xfd => 3,
        0x18 => 5,
        0xfa | 0xfb => 4,
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
        | 0xe3..=0xf9 => 1,
        _ => 2,
    }
}

/// Counts the instructions in a code item by walking opcode lengths, without
/// decoding operands or building instructions.
pub fn count_instructions(buf: &[u8], start: usize, insns_size: usize) -> Result<u32> {
    let mut pc = 0usize;
    let mut count = 0u32;
    while pc < insns_size {
        let unit_off = start + pc * 2;
        require_len(buf, unit_off, 2, "code item instruction")?;
        let unit0 = super::format::u16_at(buf, unit_off);
        let opcode = (unit0 & 0xFF) as u8;
        pc += if opcode == 0x00 {
            payload_units(buf, unit_off, unit0)?
        } else {
            opcode_units(opcode)
        };
        count += 1;
    }
    Ok(count)
}

/// Length of a `nop` or of the switch / fill-array payload it introduces.
pub(super) fn payload_units(buf: &[u8], unit_off: usize, unit0: u16) -> Result<usize> {
    use super::format::{u16_at, u32_at};
    Ok(match unit0 {
        0x0100 => {
            require_len(buf, unit_off, 4, "packed-switch payload")?;
            4 + u16_at(buf, unit_off + 2) as usize * 2
        }
        0x0200 => {
            require_len(buf, unit_off, 4, "sparse-switch payload")?;
            2 + u16_at(buf, unit_off + 2) as usize * 4
        }
        0x0300 => {
            require_len(buf, unit_off, 8, "fill-array-data payload")?;
            let data_bytes =
                u32_at(buf, unit_off + 4) as usize * u16_at(buf, unit_off + 2) as usize;
            (8 + data_bytes).div_ceil(2)
        }
        _ => 1,
    })
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
            opcode_units(opcode) * 2,
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
