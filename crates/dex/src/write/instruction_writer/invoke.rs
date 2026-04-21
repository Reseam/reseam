// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::{encode_35c, pack_aa_op, unpack_args, validate_35c_args};

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::FilledNewArray { type_, args } => {
            encode_35c(code, 0x24, type_.0 as u16, args)?;
        }

        Instruction::FilledNewArrayRange {
            type_,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x25, *count));
            code.push(type_.0 as u16);
            code.push(*first_reg);
        }

        Instruction::InvokeVirtual { method, args } => {
            encode_35c(code, 0x6e, method.0 as u16, args)?
        }
        Instruction::InvokeSuper { method, args } => encode_35c(code, 0x6f, method.0 as u16, args)?,
        Instruction::InvokeDirect { method, args } => {
            encode_35c(code, 0x70, method.0 as u16, args)?
        }
        Instruction::InvokeStatic { method, args } => {
            encode_35c(code, 0x71, method.0 as u16, args)?
        }
        Instruction::InvokeInterface { method, args } => {
            encode_35c(code, 0x72, method.0 as u16, args)?
        }

        Instruction::InvokeVirtualRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x74, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeSuperRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x75, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeDirectRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x76, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeStaticRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x77, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x78, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }

        Instruction::InvokePolymorphic {
            method,
            proto,
            args,
        } => {
            validate_35c_args(args)?;
            let count = args.len() as u8;
            let (c, d, e, f, g) = unpack_args(args);
            code.push(0xfa | ((count as u16) << 12) | ((g as u16) << 8));
            code.push(method.0 as u16);
            code.push((c as u16) | ((d as u16) << 4) | ((e as u16) << 8) | ((f as u16) << 12));
            code.push(proto.0);
        }
        Instruction::InvokePolymorphicRange {
            method,
            proto,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0xfb, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
            code.push(proto.0);
        }

        Instruction::InvokeCustom { call_site, args } => {
            encode_35c(code, 0xfc, call_site.0 as u16, args)?
        }
        Instruction::InvokeCustomRange {
            call_site,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0xfd, *count));
            code.push(call_site.0 as u16);
            code.push(*first_reg);
        }

        _ => unreachable!(),
    }
    Ok(())
}
