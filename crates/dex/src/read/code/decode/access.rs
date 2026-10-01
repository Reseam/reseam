// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::read::u16_at;
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    AGET, AGET_BOOLEAN, AGET_BYTE, AGET_CHAR, AGET_OBJECT, AGET_SHORT, AGET_WIDE, APUT,
    APUT_BOOLEAN, APUT_BYTE, APUT_CHAR, APUT_OBJECT, APUT_SHORT, APUT_WIDE, IGET, IGET_BOOLEAN,
    IGET_BYTE, IGET_CHAR, IGET_OBJECT, IGET_SHORT, IGET_WIDE, IPUT, IPUT_BOOLEAN, IPUT_BYTE,
    IPUT_CHAR, IPUT_OBJECT, IPUT_SHORT, IPUT_WIDE, SGET, SGET_BOOLEAN, SGET_BYTE, SGET_CHAR,
    SGET_OBJECT, SGET_SHORT, SGET_WIDE, SPUT, SPUT_BOOLEAN, SPUT_BYTE, SPUT_CHAR, SPUT_OBJECT,
    SPUT_SHORT, SPUT_WIDE,
};

use super::super::arithmetic::decode_23x;
use super::{hi8, nibbles};

pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, unit_off);

    match u16::from(opcode) {
        AGET..=APUT_SHORT => decode_array_access(buf, unit_off, opcode),
        IGET..=IPUT_SHORT => decode_instance_field_access(unit0, buf, unit_off, opcode),
        SGET..=SPUT_SHORT => decode_static_field_access(unit0, buf, unit_off, opcode),
        _ => Err(invalid(
            "instruction opcode",
            format!("{opcode:#x} is outside the decoded family"),
        )),
    }
}

fn decode_array_access(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let [a, b, c] = decode_23x(buf, unit_off);

    Ok(match u16::from(opcode) {
        AGET => Instruction::Aget {
            dest: a,
            array: b,
            index: c,
        },
        AGET_WIDE => Instruction::AgetWide {
            dest: a,
            array: b,
            index: c,
        },
        AGET_OBJECT => Instruction::AgetObject {
            dest: a,
            array: b,
            index: c,
        },
        AGET_BOOLEAN => Instruction::AgetBoolean {
            dest: a,
            array: b,
            index: c,
        },
        AGET_BYTE => Instruction::AgetByte {
            dest: a,
            array: b,
            index: c,
        },
        AGET_CHAR => Instruction::AgetChar {
            dest: a,
            array: b,
            index: c,
        },
        AGET_SHORT => Instruction::AgetShort {
            dest: a,
            array: b,
            index: c,
        },
        APUT => Instruction::Aput {
            src: a,
            array: b,
            index: c,
        },
        APUT_WIDE => Instruction::AputWide {
            src: a,
            array: b,
            index: c,
        },
        APUT_OBJECT => Instruction::AputObject {
            src: a,
            array: b,
            index: c,
        },
        APUT_BOOLEAN => Instruction::AputBoolean {
            src: a,
            array: b,
            index: c,
        },
        APUT_BYTE => Instruction::AputByte {
            src: a,
            array: b,
            index: c,
        },
        APUT_CHAR => Instruction::AputChar {
            src: a,
            array: b,
            index: c,
        },
        APUT_SHORT => Instruction::AputShort {
            src: a,
            array: b,
            index: c,
        },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_instance_field_access(
    unit0: u16,
    buf: &[u8],
    unit_off: usize,
    opcode: u8,
) -> Result<Instruction> {
    let (a, b) = nibbles(unit0);
    let field = crate::types::FieldIdx(u32::from(u16_at(buf, unit_off + 2)));

    Ok(match u16::from(opcode) {
        IGET => Instruction::Iget {
            dest: a,
            obj: b,
            field,
        },
        IGET_WIDE => Instruction::IgetWide {
            dest: a,
            obj: b,
            field,
        },
        IGET_OBJECT => Instruction::IgetObject {
            dest: a,
            obj: b,
            field,
        },
        IGET_BOOLEAN => Instruction::IgetBoolean {
            dest: a,
            obj: b,
            field,
        },
        IGET_BYTE => Instruction::IgetByte {
            dest: a,
            obj: b,
            field,
        },
        IGET_CHAR => Instruction::IgetChar {
            dest: a,
            obj: b,
            field,
        },
        IGET_SHORT => Instruction::IgetShort {
            dest: a,
            obj: b,
            field,
        },
        IPUT => Instruction::Iput {
            src: a,
            obj: b,
            field,
        },
        IPUT_WIDE => Instruction::IputWide {
            src: a,
            obj: b,
            field,
        },
        IPUT_OBJECT => Instruction::IputObject {
            src: a,
            obj: b,
            field,
        },
        IPUT_BOOLEAN => Instruction::IputBoolean {
            src: a,
            obj: b,
            field,
        },
        IPUT_BYTE => Instruction::IputByte {
            src: a,
            obj: b,
            field,
        },
        IPUT_CHAR => Instruction::IputChar {
            src: a,
            obj: b,
            field,
        },
        IPUT_SHORT => Instruction::IputShort {
            src: a,
            obj: b,
            field,
        },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_static_field_access(
    unit0: u16,
    buf: &[u8],
    unit_off: usize,
    opcode: u8,
) -> Result<Instruction> {
    let register = hi8(unit0);
    let field = crate::types::FieldIdx(u32::from(u16_at(buf, unit_off + 2)));

    Ok(match u16::from(opcode) {
        SGET => Instruction::Sget {
            dest: register,
            field,
        },
        SGET_WIDE => Instruction::SgetWide {
            dest: register,
            field,
        },
        SGET_OBJECT => Instruction::SgetObject {
            dest: register,
            field,
        },
        SGET_BOOLEAN => Instruction::SgetBoolean {
            dest: register,
            field,
        },
        SGET_BYTE => Instruction::SgetByte {
            dest: register,
            field,
        },
        SGET_CHAR => Instruction::SgetChar {
            dest: register,
            field,
        },
        SGET_SHORT => Instruction::SgetShort {
            dest: register,
            field,
        },
        SPUT => Instruction::Sput {
            src: register,
            field,
        },
        SPUT_WIDE => Instruction::SputWide {
            src: register,
            field,
        },
        SPUT_OBJECT => Instruction::SputObject {
            src: register,
            field,
        },
        SPUT_BOOLEAN => Instruction::SputBoolean {
            src: register,
            field,
        },
        SPUT_BYTE => Instruction::SputByte {
            src: register,
            field,
        },
        SPUT_CHAR => Instruction::SputChar {
            src: register,
            field,
        },
        SPUT_SHORT => Instruction::SputShort {
            src: register,
            field,
        },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}
