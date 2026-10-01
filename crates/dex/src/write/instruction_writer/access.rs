// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::types::instruction::Instruction;

use super::{encode_23x, pack_12x, pack_aa_op};

pub(super) fn encode_instruction(
    code: &mut Vec<u16>,
    instruction: &Instruction,
    op: u16,
) -> Result<()> {
    match instruction {
        Instruction::Aget { dest, array, index } => encode_23x(code, op, *dest, *array, *index),
        Instruction::AgetWide { dest, array, index }
        | Instruction::AgetObject { dest, array, index }
        | Instruction::AgetBoolean { dest, array, index }
        | Instruction::AgetByte { dest, array, index }
        | Instruction::AgetChar { dest, array, index }
        | Instruction::AgetShort { dest, array, index } => {
            encode_23x(code, op, *dest, *array, *index);
        }
        Instruction::Aput { src, array, index }
        | Instruction::AputWide { src, array, index }
        | Instruction::AputByte { src, array, index }
        | Instruction::AputChar { src, array, index } => encode_23x(code, op, *src, *array, *index),
        Instruction::AputObject { src, array, index }
        | Instruction::AputBoolean { src, array, index }
        | Instruction::AputShort { src, array, index } => {
            encode_23x(code, op, *src, *array, *index);
        }
        Instruction::Iget { dest, obj, field }
        | Instruction::IgetWide { dest, obj, field }
        | Instruction::IgetObject { dest, obj, field }
        | Instruction::IgetBoolean { dest, obj, field }
        | Instruction::IgetByte { dest, obj, field }
        | Instruction::IgetChar { dest, obj, field }
        | Instruction::IgetShort { dest, obj, field } => {
            code.push(pack_12x(op, *dest, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::Iput { src, obj, field }
        | Instruction::IputWide { src, obj, field }
        | Instruction::IputObject { src, obj, field }
        | Instruction::IputBoolean { src, obj, field }
        | Instruction::IputByte { src, obj, field }
        | Instruction::IputChar { src, obj, field }
        | Instruction::IputShort { src, obj, field } => {
            code.push(pack_12x(op, *src, *obj)?);
            code.push(field.0 as u16);
        }
        Instruction::Sget { dest, field }
        | Instruction::SgetWide { dest, field }
        | Instruction::SgetObject { dest, field }
        | Instruction::SgetBoolean { dest, field }
        | Instruction::SgetByte { dest, field }
        | Instruction::SgetChar { dest, field }
        | Instruction::SgetShort { dest, field } => {
            code.push(pack_aa_op(op, *dest));
            code.push(field.0 as u16);
        }
        Instruction::Sput { src, field }
        | Instruction::SputWide { src, field }
        | Instruction::SputObject { src, field }
        | Instruction::SputBoolean { src, field }
        | Instruction::SputByte { src, field }
        | Instruction::SputChar { src, field }
        | Instruction::SputShort { src, field } => {
            code.push(pack_aa_op(op, *src));
            code.push(field.0 as u16);
        }
        _ => {
            return Err(invalid(
                "instruction encoding",
                "instruction is outside the encoded family",
            ));
        }
    }
    Ok(())
}
