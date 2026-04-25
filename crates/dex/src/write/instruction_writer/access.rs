// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::{encode_23x, pack_12x, pack_aa_op};

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::Aget { dest, array, index } => encode_23x(code, 0x44, *dest, *array, *index),
        Instruction::AgetWide { dest, array, index } => {
            encode_23x(code, 0x45, *dest, *array, *index)
        }
        Instruction::AgetObject { dest, array, index } => {
            encode_23x(code, 0x46, *dest, *array, *index)
        }
        Instruction::AgetBoolean { dest, array, index } => {
            encode_23x(code, 0x47, *dest, *array, *index)
        }
        Instruction::AgetByte { dest, array, index } => {
            encode_23x(code, 0x48, *dest, *array, *index)
        }
        Instruction::AgetChar { dest, array, index } => {
            encode_23x(code, 0x49, *dest, *array, *index)
        }
        Instruction::AgetShort { dest, array, index } => {
            encode_23x(code, 0x4a, *dest, *array, *index)
        }
        Instruction::Aput { src, array, index } => encode_23x(code, 0x4b, *src, *array, *index),
        Instruction::AputWide { src, array, index } => encode_23x(code, 0x4c, *src, *array, *index),
        Instruction::AputObject { src, array, index } => {
            encode_23x(code, 0x4d, *src, *array, *index)
        }
        Instruction::AputBoolean { src, array, index } => {
            encode_23x(code, 0x4e, *src, *array, *index)
        }
        Instruction::AputByte { src, array, index } => encode_23x(code, 0x4f, *src, *array, *index),
        Instruction::AputChar { src, array, index } => encode_23x(code, 0x50, *src, *array, *index),
        Instruction::AputShort { src, array, index } => {
            encode_23x(code, 0x51, *src, *array, *index)
        }

        Instruction::Iget { dest, obj, field } => {
            code.push(pack_12x(0x52, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetWide { dest, obj, field } => {
            code.push(pack_12x(0x53, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetObject { dest, obj, field } => {
            code.push(pack_12x(0x54, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetBoolean { dest, obj, field } => {
            code.push(pack_12x(0x55, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetByte { dest, obj, field } => {
            code.push(pack_12x(0x56, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetChar { dest, obj, field } => {
            code.push(pack_12x(0x57, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IgetShort { dest, obj, field } => {
            code.push(pack_12x(0x58, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::Iput { src, obj, field } => {
            code.push(pack_12x(0x59, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputWide { src, obj, field } => {
            code.push(pack_12x(0x5a, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputObject { src, obj, field } => {
            code.push(pack_12x(0x5b, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputBoolean { src, obj, field } => {
            code.push(pack_12x(0x5c, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputByte { src, obj, field } => {
            code.push(pack_12x(0x5d, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputChar { src, obj, field } => {
            code.push(pack_12x(0x5e, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::IputShort { src, obj, field } => {
            code.push(pack_12x(0x5f, *src, *obj)?);
            code.push(field.0 as u16);
        }

        Instruction::Sget { dest, field } => {
            code.push(pack_aa_op(0x60, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetWide { dest, field } => {
            code.push(pack_aa_op(0x61, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetObject { dest, field } => {
            code.push(pack_aa_op(0x62, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetBoolean { dest, field } => {
            code.push(pack_aa_op(0x63, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetByte { dest, field } => {
            code.push(pack_aa_op(0x64, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetChar { dest, field } => {
            code.push(pack_aa_op(0x65, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetShort { dest, field } => {
            code.push(pack_aa_op(0x66, *dest));
            code.push(field.0 as u16);
        }
        Instruction::Sput { src, field } => {
            code.push(pack_aa_op(0x67, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputWide { src, field } => {
            code.push(pack_aa_op(0x68, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputObject { src, field } => {
            code.push(pack_aa_op(0x69, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputBoolean { src, field } => {
            code.push(pack_aa_op(0x6a, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputByte { src, field } => {
            code.push(pack_aa_op(0x6b, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputChar { src, field } => {
            code.push(pack_aa_op(0x6c, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputShort { src, field } => {
            code.push(pack_aa_op(0x6d, *src));
            code.push(field.0 as u16);
        }

        _ => unreachable!(),
    }
    Ok(())
}
