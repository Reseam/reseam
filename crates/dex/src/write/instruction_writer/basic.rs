// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::types::instruction::Instruction;

use super::{pack_12x, pack_aa_op, validate_u4_register};

#[expect(
    clippy::too_many_lines,
    reason = "the exhaustive format dispatch keeps each encoding visible in one match"
)]
pub(super) fn encode_instruction(
    code: &mut Vec<u16>,
    instruction: &Instruction,
    op: u16,
) -> Result<()> {
    match instruction {
        Instruction::Nop | Instruction::ReturnVoid => code.push(op),
        Instruction::Move { dest, src }
        | Instruction::MoveWide { dest, src }
        | Instruction::MoveObject { dest, src } => code.push(pack_12x(op, *dest, *src)?),
        Instruction::ArrayLength { dest, array } => code.push(pack_12x(op, *dest, *array)?),
        Instruction::MoveFrom16 { dest, src }
        | Instruction::MoveWideFrom16 { dest, src }
        | Instruction::MoveObjectFrom16 { dest, src } => {
            code.push(pack_aa_op(op, *dest));
            code.push(*src);
        }
        Instruction::Move16 { dest, src }
        | Instruction::MoveWide16 { dest, src }
        | Instruction::MoveObject16 { dest, src } => {
            code.push(op);
            code.push(*dest);
            code.push(*src);
        }
        Instruction::MoveResult { dest }
        | Instruction::MoveResultWide { dest }
        | Instruction::MoveResultObject { dest }
        | Instruction::MoveException { dest } => code.push(pack_aa_op(op, *dest)),
        Instruction::Return { src }
        | Instruction::ReturnWide { src }
        | Instruction::ReturnObject { src } => code.push(pack_aa_op(op, *src)),
        Instruction::Const4 { dest, value } => {
            validate_u4_register(*dest, "A")?;
            if !(-8..=7).contains(value) {
                return Err(invalid(
                    "instruction",
                    format!("literal {value} does not fit const/4 signed nibble range (-8..7)"),
                ));
            }
            let value = (*value as u8) & 0xF;
            code.push(op | (u16::from(*dest) << 8) | (u16::from(value) << 12));
        }
        Instruction::Const16 { dest, value }
        | Instruction::ConstWide16 { dest, value }
        | Instruction::ConstHigh16 { dest, value }
        | Instruction::ConstWideHigh16 { dest, value } => {
            code.push(pack_aa_op(op, *dest));
            code.push(*value as u16);
        }
        Instruction::Const { dest, value } | Instruction::ConstWide32 { dest, value } => {
            code.push(pack_aa_op(op, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
        }
        Instruction::ConstWide { dest, value } => {
            code.push(pack_aa_op(op, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
            code.push((*value >> 32) as u16);
            code.push((*value >> 48) as u16);
        }
        Instruction::ConstString { dest, string } => {
            code.push(pack_aa_op(op, *dest));
            code.push(string.0 as u16);
        }
        Instruction::ConstClass { dest, type_ } | Instruction::NewInstance { dest, type_ } => {
            code.push(pack_aa_op(op, *dest));
            code.push(type_.0 as u16);
        }
        Instruction::ConstMethodHandle {
            dest,
            method_handle,
        } => {
            code.push(pack_aa_op(op, *dest));
            code.push(method_handle.0 as u16);
        }
        Instruction::ConstMethodType { dest, proto } => {
            code.push(pack_aa_op(op, *dest));
            code.push(
                u16::try_from(proto.0)
                    .map_err(|_| invalid("instruction", "prototype index exceeds encoded width"))?,
            );
        }
        Instruction::ConstStringJumbo { dest, string } => {
            code.push(pack_aa_op(op, *dest));
            code.push(string.0 as u16);
            code.push((string.0 >> 16) as u16);
        }
        Instruction::MonitorEnter { ref_ } | Instruction::MonitorExit { ref_ } => {
            code.push(pack_aa_op(op, *ref_));
        }
        Instruction::CheckCast { ref_, type_ } => {
            code.push(pack_aa_op(op, *ref_));
            code.push(type_.0 as u16);
        }
        Instruction::InstanceOf { dest, ref_, type_ } => {
            code.push(pack_12x(op, *dest, *ref_)?);
            code.push(type_.0 as u16);
        }
        Instruction::NewArray { dest, size, type_ } => {
            code.push(pack_12x(op, *dest, *size)?);
            code.push(type_.0 as u16);
        }
        Instruction::FillArrayData {
            array,
            payload_offset,
        } => {
            code.push(pack_aa_op(op, *array));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }
        Instruction::Throw { exception } => code.push(pack_aa_op(op, *exception)),
        Instruction::Goto { offset } => code.push(pack_aa_op(op, *offset as u8)),
        Instruction::Goto16 { offset } => {
            code.push(op);
            code.push(*offset as u16);
        }
        Instruction::Goto32 { offset } => {
            code.push(op);
            code.push(*offset as u16);
            code.push((*offset >> 16) as u16);
        }
        Instruction::PackedSwitch {
            test,
            payload_offset,
        }
        | Instruction::SparseSwitch {
            test,
            payload_offset,
        } => {
            code.push(pack_aa_op(op, *test));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }
        Instruction::IfEq { a, b, offset }
        | Instruction::IfNe { a, b, offset }
        | Instruction::IfLt { a, b, offset }
        | Instruction::IfGe { a, b, offset }
        | Instruction::IfGt { a, b, offset }
        | Instruction::IfLe { a, b, offset } => {
            code.push(pack_12x(op, *a, *b)?);
            code.push(*offset as u16);
        }
        Instruction::IfEqz { a, offset }
        | Instruction::IfNez { a, offset }
        | Instruction::IfLtz { a, offset }
        | Instruction::IfGez { a, offset }
        | Instruction::IfGtz { a, offset }
        | Instruction::IfLez { a, offset } => {
            code.push(pack_aa_op(op, *a));
            code.push(*offset as u16);
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
