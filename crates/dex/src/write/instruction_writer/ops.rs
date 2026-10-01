// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::types::instruction::Instruction;

use super::{encode_23x, pack_12x, pack_aa_op};

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
        Instruction::CmpLFloat { dest, a, b }
        | Instruction::CmpGFloat { dest, a, b }
        | Instruction::CmpLDouble { dest, a, b }
        | Instruction::CmpGDouble { dest, a, b }
        | Instruction::CmpLong { dest, a, b }
        | Instruction::AddInt { dest, a, b }
        | Instruction::SubInt { dest, a, b }
        | Instruction::MulInt { dest, a, b }
        | Instruction::DivInt { dest, a, b }
        | Instruction::RemInt { dest, a, b }
        | Instruction::AndInt { dest, a, b }
        | Instruction::OrInt { dest, a, b }
        | Instruction::XorInt { dest, a, b }
        | Instruction::ShlInt { dest, a, b }
        | Instruction::ShrInt { dest, a, b }
        | Instruction::UshrInt { dest, a, b }
        | Instruction::AddLong { dest, a, b }
        | Instruction::SubLong { dest, a, b }
        | Instruction::MulLong { dest, a, b }
        | Instruction::DivLong { dest, a, b }
        | Instruction::RemLong { dest, a, b }
        | Instruction::AndLong { dest, a, b }
        | Instruction::OrLong { dest, a, b }
        | Instruction::XorLong { dest, a, b }
        | Instruction::ShlLong { dest, a, b }
        | Instruction::ShrLong { dest, a, b }
        | Instruction::UshrLong { dest, a, b }
        | Instruction::AddFloat { dest, a, b }
        | Instruction::SubFloat { dest, a, b }
        | Instruction::MulFloat { dest, a, b }
        | Instruction::DivFloat { dest, a, b }
        | Instruction::RemFloat { dest, a, b }
        | Instruction::AddDouble { dest, a, b }
        | Instruction::SubDouble { dest, a, b }
        | Instruction::MulDouble { dest, a, b }
        | Instruction::DivDouble { dest, a, b }
        | Instruction::RemDouble { dest, a, b } => encode_23x(code, op, *dest, *a, *b),
        Instruction::NegInt { dest, src }
        | Instruction::NotInt { dest, src }
        | Instruction::NegLong { dest, src }
        | Instruction::NotLong { dest, src }
        | Instruction::NegFloat { dest, src }
        | Instruction::NegDouble { dest, src }
        | Instruction::IntToLong { dest, src }
        | Instruction::IntToFloat { dest, src }
        | Instruction::IntToDouble { dest, src }
        | Instruction::LongToInt { dest, src }
        | Instruction::LongToFloat { dest, src }
        | Instruction::LongToDouble { dest, src }
        | Instruction::FloatToInt { dest, src }
        | Instruction::FloatToLong { dest, src }
        | Instruction::FloatToDouble { dest, src }
        | Instruction::DoubleToInt { dest, src }
        | Instruction::DoubleToLong { dest, src }
        | Instruction::DoubleToFloat { dest, src }
        | Instruction::IntToByte { dest, src }
        | Instruction::IntToChar { dest, src }
        | Instruction::IntToShort { dest, src } => code.push(pack_12x(op, *dest, *src)?),
        Instruction::AddInt2Addr { dest_a, b }
        | Instruction::SubInt2Addr { dest_a, b }
        | Instruction::MulInt2Addr { dest_a, b }
        | Instruction::DivInt2Addr { dest_a, b }
        | Instruction::RemInt2Addr { dest_a, b }
        | Instruction::AndInt2Addr { dest_a, b }
        | Instruction::OrInt2Addr { dest_a, b }
        | Instruction::XorInt2Addr { dest_a, b }
        | Instruction::ShlInt2Addr { dest_a, b }
        | Instruction::ShrInt2Addr { dest_a, b }
        | Instruction::UshrInt2Addr { dest_a, b }
        | Instruction::AddLong2Addr { dest_a, b }
        | Instruction::SubLong2Addr { dest_a, b }
        | Instruction::MulLong2Addr { dest_a, b }
        | Instruction::DivLong2Addr { dest_a, b }
        | Instruction::RemLong2Addr { dest_a, b }
        | Instruction::AndLong2Addr { dest_a, b }
        | Instruction::OrLong2Addr { dest_a, b }
        | Instruction::XorLong2Addr { dest_a, b }
        | Instruction::ShlLong2Addr { dest_a, b }
        | Instruction::ShrLong2Addr { dest_a, b }
        | Instruction::UshrLong2Addr { dest_a, b }
        | Instruction::AddFloat2Addr { dest_a, b }
        | Instruction::SubFloat2Addr { dest_a, b }
        | Instruction::MulFloat2Addr { dest_a, b }
        | Instruction::DivFloat2Addr { dest_a, b }
        | Instruction::RemFloat2Addr { dest_a, b }
        | Instruction::AddDouble2Addr { dest_a, b }
        | Instruction::SubDouble2Addr { dest_a, b }
        | Instruction::MulDouble2Addr { dest_a, b }
        | Instruction::DivDouble2Addr { dest_a, b }
        | Instruction::RemDouble2Addr { dest_a, b } => code.push(pack_12x(op, *dest_a, *b)?),
        Instruction::AddIntLit16 { dest, src, literal }
        | Instruction::RsubIntLit16 { dest, src, literal }
        | Instruction::MulIntLit16 { dest, src, literal }
        | Instruction::DivIntLit16 { dest, src, literal }
        | Instruction::RemIntLit16 { dest, src, literal }
        | Instruction::AndIntLit16 { dest, src, literal }
        | Instruction::OrIntLit16 { dest, src, literal }
        | Instruction::XorIntLit16 { dest, src, literal } => {
            code.push(pack_12x(op, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::AddIntLit8 { dest, src, literal }
        | Instruction::RsubIntLit8 { dest, src, literal }
        | Instruction::MulIntLit8 { dest, src, literal }
        | Instruction::DivIntLit8 { dest, src, literal }
        | Instruction::RemIntLit8 { dest, src, literal }
        | Instruction::AndIntLit8 { dest, src, literal }
        | Instruction::OrIntLit8 { dest, src, literal }
        | Instruction::XorIntLit8 { dest, src, literal }
        | Instruction::ShlIntLit8 { dest, src, literal }
        | Instruction::ShrIntLit8 { dest, src, literal }
        | Instruction::UshrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(op, *dest));
            code.push(u16::from(*src) | (u16::from(*literal as u8) << 8));
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
