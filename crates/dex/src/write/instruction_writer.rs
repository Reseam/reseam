// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    AGET, CMP_L_FLOAT, CMP_LONG, CONST_METHOD_TYPE, FILLED_NEW_ARRAY, FILLED_NEW_ARRAY_RANGE,
    INVOKE_CUSTOM_RANGE, INVOKE_INTERFACE_RANGE, INVOKE_POLYMORPHIC, INVOKE_VIRTUAL, NEG_INT, NOP,
    SPUT_SHORT, USHR_INT_LIT8,
};

mod access;
mod basic;
mod invoke;
mod ops;
mod payloads;

pub fn encode_instructions(instructions: &[Instruction]) -> Result<Vec<u16>> {
    let capacity = instructions
        .iter()
        .map(|instruction| instruction.code_units() as usize)
        .sum();
    let mut code = Vec::with_capacity(capacity);
    for instruction in instructions {
        encode_instruction(&mut code, instruction)?;
    }
    Ok(code)
}

fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction.opcode() {
        Some(
            op @ (FILLED_NEW_ARRAY..=FILLED_NEW_ARRAY_RANGE
            | INVOKE_VIRTUAL..=INVOKE_INTERFACE_RANGE
            | INVOKE_POLYMORPHIC..=INVOKE_CUSTOM_RANGE),
        ) => invoke::encode_instruction(code, instruction, op),
        Some(op @ (CMP_L_FLOAT..=CMP_LONG | NEG_INT..=USHR_INT_LIT8)) => {
            ops::encode_instruction(code, instruction, op)
        }
        Some(op @ (AGET..=SPUT_SHORT)) => access::encode_instruction(code, instruction, op),
        Some(op @ (NOP..=CONST_METHOD_TYPE)) => basic::encode_instruction(code, instruction, op),
        _ => payloads::encode_instruction(code, instruction),
    }
}

pub(super) fn pack_aa_op(op: u16, aa: u8) -> u16 {
    op | (u16::from(aa) << 8)
}

pub(super) fn pack_12x(op: u16, a: u8, b: u8) -> Result<u16> {
    validate_u4_register(a, "A")?;
    validate_u4_register(b, "B")?;
    Ok(op | (u16::from(a) << 8) | (u16::from(b) << 12))
}

pub(super) fn encode_23x(code: &mut Vec<u16>, op: u16, aa: u8, bb: u8, cc: u8) {
    code.push(op | (u16::from(aa) << 8));
    code.push(u16::from(bb) | (u16::from(cc) << 8));
}

pub(super) fn encode_35c(code: &mut Vec<u16>, op: u16, idx: u16, args: &[u8]) -> Result<()> {
    validate_35c_args(args)?;
    let count = args.len() as u8;
    let [arg0, arg1, arg2, arg3, arg4] = unpack_args(args);
    code.push(op | (u16::from(count) << 12) | (u16::from(arg4) << 8));
    code.push(idx);
    code.push(
        u16::from(arg0) | (u16::from(arg1) << 4) | (u16::from(arg2) << 8) | (u16::from(arg3) << 12),
    );
    Ok(())
}

pub(super) fn validate_35c_args(args: &[u8]) -> Result<()> {
    if args.len() > 5 {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register count {} exceeds maximum 5 for format 35c/45cc — \
                 use the range variant instead",
                args.len()
            ),
        ));
    }
    if let Some(&register) = args.iter().find(|&&register| register > 15) {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register v{register} exceeds nibble range (0-15) for format 35c/45cc — \
                 use the range variant instead"
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_u4_register(register: u8, operand: &str) -> Result<()> {
    if register > 15 {
        return Err(crate::error::invalid(
            "instruction",
            format!("register v{register} exceeds nibble range (0-15) for operand {operand}"),
        ));
    }
    Ok(())
}

pub(super) fn unpack_args(args: &[u8]) -> [u8; 5] {
    std::array::from_fn(|index| args.get(index).copied().unwrap_or(0))
}
