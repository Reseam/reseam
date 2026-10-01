// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::types::instruction::Instruction;

use super::{encode_35c, pack_aa_op};

pub(super) fn encode_instruction(
    code: &mut Vec<u16>,
    instruction: &Instruction,
    op: u16,
) -> Result<()> {
    match instruction {
        Instruction::FilledNewArray { type_, args } => {
            encode_35c(code, op, type_.0 as u16, args)?;
        }
        Instruction::FilledNewArrayRange {
            type_,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(op, *count));
            code.push(type_.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeVirtual { method, args }
        | Instruction::InvokeDirect { method, args }
        | Instruction::InvokeStatic { method, args }
        | Instruction::InvokeInterface { method, args } => {
            encode_35c(code, op, method.0 as u16, args)?;
        }
        Instruction::InvokeSuper { method, args } => encode_35c(code, op, method.0 as u16, args)?,
        Instruction::InvokeVirtualRange {
            method,
            first_reg,
            count,
        }
        | Instruction::InvokeSuperRange {
            method,
            first_reg,
            count,
        }
        | Instruction::InvokeDirectRange {
            method,
            first_reg,
            count,
        }
        | Instruction::InvokeStaticRange {
            method,
            first_reg,
            count,
        }
        | Instruction::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(op, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokePolymorphic {
            method,
            proto,
            args,
        } => {
            encode_35c(code, op, method.0 as u16, args)?;
            code.push(
                u16::try_from(proto.0)
                    .map_err(|_| invalid("instruction", "prototype index exceeds encoded width"))?,
            );
        }
        Instruction::InvokePolymorphicRange {
            method,
            proto,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(op, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
            code.push(
                u16::try_from(proto.0)
                    .map_err(|_| invalid("instruction", "prototype index exceeds encoded width"))?,
            );
        }
        Instruction::InvokeCustom { call_site, args } => {
            encode_35c(code, op, call_site.0 as u16, args)?;
        }
        Instruction::InvokeCustomRange {
            call_site,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(op, *count));
            code.push(call_site.0 as u16);
            code.push(*first_reg);
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
