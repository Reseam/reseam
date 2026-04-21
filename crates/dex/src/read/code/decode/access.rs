// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::super::arithmetic::decode_23x;
use super::super::format::u16_at;
use super::{hi8, nibbles, DecodedInstruction};

pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<DecodedInstruction> {
    let unit0 = u16_at(buf, unit_off);
    let decoded = match opcode {
        0x44..=0x51 => decode_array_access(buf, unit_off, opcode),
        0x52..=0x5f => decode_instance_field_access(unit0, buf, unit_off, opcode),
        0x60..=0x6d => decode_static_field_access(unit0, buf, unit_off, opcode),
        _ => unreachable!(),
    };
    Ok(decoded)
}

fn decode_array_access(buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let (a, b, c) = decode_23x(buf, unit_off);
    let instruction = match opcode {
        0x44 => Instruction::Aget {
            dest: a,
            array: b,
            index: c,
        },
        0x45 => Instruction::AgetWide {
            dest: a,
            array: b,
            index: c,
        },
        0x46 => Instruction::AgetObject {
            dest: a,
            array: b,
            index: c,
        },
        0x47 => Instruction::AgetBoolean {
            dest: a,
            array: b,
            index: c,
        },
        0x48 => Instruction::AgetByte {
            dest: a,
            array: b,
            index: c,
        },
        0x49 => Instruction::AgetChar {
            dest: a,
            array: b,
            index: c,
        },
        0x4a => Instruction::AgetShort {
            dest: a,
            array: b,
            index: c,
        },
        0x4b => Instruction::Aput {
            src: a,
            array: b,
            index: c,
        },
        0x4c => Instruction::AputWide {
            src: a,
            array: b,
            index: c,
        },
        0x4d => Instruction::AputObject {
            src: a,
            array: b,
            index: c,
        },
        0x4e => Instruction::AputBoolean {
            src: a,
            array: b,
            index: c,
        },
        0x4f => Instruction::AputByte {
            src: a,
            array: b,
            index: c,
        },
        0x50 => Instruction::AputChar {
            src: a,
            array: b,
            index: c,
        },
        0x51 => Instruction::AputShort {
            src: a,
            array: b,
            index: c,
        },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_instance_field_access(
    unit0: u16,
    buf: &[u8],
    unit_off: usize,
    opcode: u8,
) -> DecodedInstruction {
    let (a, b) = nibbles(unit0);
    let field = crate::types::FieldIdx(u16_at(buf, unit_off + 2) as u32);
    let instruction = match opcode {
        0x52 => Instruction::Iget {
            dest: a,
            obj: b,
            field,
        },
        0x53 => Instruction::IgetWide {
            dest: a,
            obj: b,
            field,
        },
        0x54 => Instruction::IgetObject {
            dest: a,
            obj: b,
            field,
        },
        0x55 => Instruction::IgetBoolean {
            dest: a,
            obj: b,
            field,
        },
        0x56 => Instruction::IgetByte {
            dest: a,
            obj: b,
            field,
        },
        0x57 => Instruction::IgetChar {
            dest: a,
            obj: b,
            field,
        },
        0x58 => Instruction::IgetShort {
            dest: a,
            obj: b,
            field,
        },
        0x59 => Instruction::Iput {
            src: a,
            obj: b,
            field,
        },
        0x5a => Instruction::IputWide {
            src: a,
            obj: b,
            field,
        },
        0x5b => Instruction::IputObject {
            src: a,
            obj: b,
            field,
        },
        0x5c => Instruction::IputBoolean {
            src: a,
            obj: b,
            field,
        },
        0x5d => Instruction::IputByte {
            src: a,
            obj: b,
            field,
        },
        0x5e => Instruction::IputChar {
            src: a,
            obj: b,
            field,
        },
        0x5f => Instruction::IputShort {
            src: a,
            obj: b,
            field,
        },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_static_field_access(
    unit0: u16,
    buf: &[u8],
    unit_off: usize,
    opcode: u8,
) -> DecodedInstruction {
    let register = hi8(unit0);
    let field = crate::types::FieldIdx(u16_at(buf, unit_off + 2) as u32);
    let instruction = match opcode {
        0x60 => Instruction::Sget {
            dest: register,
            field,
        },
        0x61 => Instruction::SgetWide {
            dest: register,
            field,
        },
        0x62 => Instruction::SgetObject {
            dest: register,
            field,
        },
        0x63 => Instruction::SgetBoolean {
            dest: register,
            field,
        },
        0x64 => Instruction::SgetByte {
            dest: register,
            field,
        },
        0x65 => Instruction::SgetChar {
            dest: register,
            field,
        },
        0x66 => Instruction::SgetShort {
            dest: register,
            field,
        },
        0x67 => Instruction::Sput {
            src: register,
            field,
        },
        0x68 => Instruction::SputWide {
            src: register,
            field,
        },
        0x69 => Instruction::SputObject {
            src: register,
            field,
        },
        0x6a => Instruction::SputBoolean {
            src: register,
            field,
        },
        0x6b => Instruction::SputByte {
            src: register,
            field,
        },
        0x6c => Instruction::SputChar {
            src: register,
            field,
        },
        0x6d => Instruction::SputShort {
            src: register,
            field,
        },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}
