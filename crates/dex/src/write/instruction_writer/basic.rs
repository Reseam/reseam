// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::{pack_12x, pack_aa_op};

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::Nop => code.push(0x0000),

        Instruction::Move { dest, src } => code.push(pack_12x(0x01, *dest, *src)),
        Instruction::MoveWide { dest, src } => code.push(pack_12x(0x04, *dest, *src)),
        Instruction::MoveObject { dest, src } => code.push(pack_12x(0x07, *dest, *src)),
        Instruction::ArrayLength { dest, array } => code.push(pack_12x(0x21, *dest, *array)),

        Instruction::MoveFrom16 { dest, src } => {
            code.push(pack_aa_op(0x02, *dest));
            code.push(*src);
        }
        Instruction::MoveWideFrom16 { dest, src } => {
            code.push(pack_aa_op(0x05, *dest));
            code.push(*src);
        }
        Instruction::MoveObjectFrom16 { dest, src } => {
            code.push(pack_aa_op(0x08, *dest));
            code.push(*src);
        }

        Instruction::Move16 { dest, src } => {
            code.push(0x03);
            code.push(*dest);
            code.push(*src);
        }
        Instruction::MoveWide16 { dest, src } => {
            code.push(0x06);
            code.push(*dest);
            code.push(*src);
        }
        Instruction::MoveObject16 { dest, src } => {
            code.push(0x09);
            code.push(*dest);
            code.push(*src);
        }

        Instruction::MoveResult { dest } => code.push(pack_aa_op(0x0a, *dest)),
        Instruction::MoveResultWide { dest } => code.push(pack_aa_op(0x0b, *dest)),
        Instruction::MoveResultObject { dest } => code.push(pack_aa_op(0x0c, *dest)),
        Instruction::MoveException { dest } => code.push(pack_aa_op(0x0d, *dest)),

        Instruction::ReturnVoid => code.push(0x0e),
        Instruction::Return { src } => code.push(pack_aa_op(0x0f, *src)),
        Instruction::ReturnWide { src } => code.push(pack_aa_op(0x10, *src)),
        Instruction::ReturnObject { src } => code.push(pack_aa_op(0x11, *src)),

        Instruction::Const4 { dest, value } => {
            let value = (*value as u8) & 0xF;
            code.push(0x12 | ((*dest as u16) << 8) | ((value as u16) << 12));
        }

        Instruction::Const16 { dest, value } => {
            code.push(pack_aa_op(0x13, *dest));
            code.push(*value as u16);
        }
        Instruction::ConstWide16 { dest, value } => {
            code.push(pack_aa_op(0x16, *dest));
            code.push(*value as u16);
        }

        Instruction::Const { dest, value } => {
            code.push(pack_aa_op(0x14, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
        }
        Instruction::ConstWide32 { dest, value } => {
            code.push(pack_aa_op(0x17, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
        }

        Instruction::ConstHigh16 { dest, value } => {
            code.push(pack_aa_op(0x15, *dest));
            code.push(*value as u16);
        }
        Instruction::ConstWideHigh16 { dest, value } => {
            code.push(pack_aa_op(0x19, *dest));
            code.push(*value as u16);
        }

        Instruction::ConstWide { dest, value } => {
            code.push(pack_aa_op(0x18, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
            code.push((*value >> 32) as u16);
            code.push((*value >> 48) as u16);
        }

        Instruction::ConstString { dest, string } => {
            code.push(pack_aa_op(0x1a, *dest));
            code.push(string.0 as u16);
        }
        Instruction::ConstClass { dest, type_ } => {
            code.push(pack_aa_op(0x1c, *dest));
            code.push(type_.0 as u16);
        }
        Instruction::ConstMethodHandle {
            dest,
            method_handle,
        } => {
            code.push(pack_aa_op(0xfe, *dest));
            code.push(method_handle.0 as u16);
        }
        Instruction::ConstMethodType { dest, proto } => {
            code.push(pack_aa_op(0xff, *dest));
            code.push(proto.0);
        }

        Instruction::ConstStringJumbo { dest, string } => {
            code.push(pack_aa_op(0x1b, *dest));
            code.push(string.0 as u16);
            code.push((string.0 >> 16) as u16);
        }

        Instruction::MonitorEnter { ref_ } => code.push(pack_aa_op(0x1d, *ref_)),
        Instruction::MonitorExit { ref_ } => code.push(pack_aa_op(0x1e, *ref_)),

        Instruction::CheckCast { ref_, type_ } => {
            code.push(pack_aa_op(0x1f, *ref_));
            code.push(type_.0 as u16);
        }

        Instruction::InstanceOf { dest, ref_, type_ } => {
            code.push(pack_12x(0x20, *dest, *ref_));
            code.push(type_.0 as u16);
        }
        Instruction::NewInstance { dest, type_ } => {
            code.push(pack_aa_op(0x22, *dest));
            code.push(type_.0 as u16);
        }
        Instruction::NewArray { dest, size, type_ } => {
            code.push(pack_12x(0x23, *dest, *size));
            code.push(type_.0 as u16);
        }

        Instruction::FillArrayData {
            array,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x26, *array));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }

        Instruction::Throw { exception } => code.push(pack_aa_op(0x27, *exception)),

        Instruction::Goto { offset } => code.push(pack_aa_op(0x28, *offset as u8)),
        Instruction::Goto16 { offset } => {
            code.push(0x29);
            code.push(*offset as u16);
        }
        Instruction::Goto32 { offset } => {
            code.push(0x2a);
            code.push(*offset as u16);
            code.push((*offset >> 16) as u16);
        }

        Instruction::PackedSwitch {
            test,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x2b, *test));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }
        Instruction::SparseSwitch {
            test,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x2c, *test));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }

        Instruction::IfEq { a, b, offset } => {
            code.push(pack_12x(0x32, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfNe { a, b, offset } => {
            code.push(pack_12x(0x33, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfLt { a, b, offset } => {
            code.push(pack_12x(0x34, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfGe { a, b, offset } => {
            code.push(pack_12x(0x35, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfGt { a, b, offset } => {
            code.push(pack_12x(0x36, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfLe { a, b, offset } => {
            code.push(pack_12x(0x37, *a, *b));
            code.push(*offset as u16);
        }

        Instruction::IfEqz { a, offset } => {
            code.push(pack_aa_op(0x38, *a));
            code.push(*offset as u16);
        }
        Instruction::IfNez { a, offset } => {
            code.push(pack_aa_op(0x39, *a));
            code.push(*offset as u16);
        }
        Instruction::IfLtz { a, offset } => {
            code.push(pack_aa_op(0x3a, *a));
            code.push(*offset as u16);
        }
        Instruction::IfGez { a, offset } => {
            code.push(pack_aa_op(0x3b, *a));
            code.push(*offset as u16);
        }
        Instruction::IfGtz { a, offset } => {
            code.push(pack_aa_op(0x3c, *a));
            code.push(*offset as u16);
        }
        Instruction::IfLez { a, offset } => {
            code.push(pack_aa_op(0x3d, *a));
            code.push(*offset as u16);
        }

        _ => unreachable!(),
    }
    Ok(())
}
