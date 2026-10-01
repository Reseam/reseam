// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::read::u16_at;
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    ADD_DOUBLE, ADD_DOUBLE2_ADDR, ADD_FLOAT, ADD_FLOAT2_ADDR, ADD_INT, ADD_INT_LIT8, ADD_INT_LIT16,
    ADD_INT2_ADDR, ADD_LONG, ADD_LONG2_ADDR, AND_INT, AND_INT_LIT8, AND_INT_LIT16, AND_INT2_ADDR,
    AND_LONG, AND_LONG2_ADDR, DIV_DOUBLE, DIV_DOUBLE2_ADDR, DIV_FLOAT, DIV_FLOAT2_ADDR, DIV_INT,
    DIV_INT_LIT8, DIV_INT_LIT16, DIV_INT2_ADDR, DIV_LONG, DIV_LONG2_ADDR, DOUBLE_TO_FLOAT,
    DOUBLE_TO_INT, DOUBLE_TO_LONG, FLOAT_TO_DOUBLE, FLOAT_TO_INT, FLOAT_TO_LONG, INT_TO_BYTE,
    INT_TO_CHAR, INT_TO_DOUBLE, INT_TO_FLOAT, INT_TO_LONG, INT_TO_SHORT, LONG_TO_DOUBLE,
    LONG_TO_FLOAT, LONG_TO_INT, MUL_DOUBLE, MUL_DOUBLE2_ADDR, MUL_FLOAT, MUL_FLOAT2_ADDR, MUL_INT,
    MUL_INT_LIT8, MUL_INT_LIT16, MUL_INT2_ADDR, MUL_LONG, MUL_LONG2_ADDR, NEG_DOUBLE, NEG_FLOAT,
    NEG_INT, NEG_LONG, NOT_INT, NOT_LONG, OR_INT, OR_INT_LIT8, OR_INT_LIT16, OR_INT2_ADDR, OR_LONG,
    OR_LONG2_ADDR, REM_DOUBLE, REM_DOUBLE2_ADDR, REM_FLOAT, REM_FLOAT2_ADDR, REM_INT, REM_INT_LIT8,
    REM_INT_LIT16, REM_INT2_ADDR, REM_LONG, REM_LONG2_ADDR, RSUB_INT_LIT8, RSUB_INT_LIT16, SHL_INT,
    SHL_INT_LIT8, SHL_INT2_ADDR, SHL_LONG, SHL_LONG2_ADDR, SHR_INT, SHR_INT_LIT8, SHR_INT2_ADDR,
    SHR_LONG, SHR_LONG2_ADDR, SUB_DOUBLE, SUB_DOUBLE2_ADDR, SUB_FLOAT, SUB_FLOAT2_ADDR, SUB_INT,
    SUB_INT2_ADDR, SUB_LONG, SUB_LONG2_ADDR, USHR_INT, USHR_INT_LIT8, USHR_INT2_ADDR, USHR_LONG,
    USHR_LONG2_ADDR, XOR_INT, XOR_INT_LIT8, XOR_INT_LIT16, XOR_INT2_ADDR, XOR_LONG, XOR_LONG2_ADDR,
};

use super::super::arithmetic::decode_23x;
use super::{hi8, nibbles};

pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, unit_off);

    match u16::from(opcode) {
        NEG_INT..=INT_TO_SHORT => decode_unary(unit0, opcode),
        ADD_INT..=REM_DOUBLE => decode_binary(buf, unit_off, opcode),
        ADD_INT2_ADDR..=REM_DOUBLE2_ADDR => decode_binary_2addr(unit0, opcode),
        ADD_INT_LIT16..=XOR_INT_LIT16 => decode_binary_lit16(unit0, buf, unit_off, opcode),
        ADD_INT_LIT8..=USHR_INT_LIT8 => decode_binary_lit8(unit0, buf, unit_off, opcode),
        _ => Err(invalid(
            "instruction opcode",
            format!("{opcode:#x} is outside the decoded family"),
        )),
    }
}

fn decode_unary(unit0: u16, opcode: u8) -> Result<Instruction> {
    let (dest, src) = nibbles(unit0);

    Ok(match u16::from(opcode) {
        NEG_INT => Instruction::NegInt { dest, src },
        NOT_INT => Instruction::NotInt { dest, src },
        NEG_LONG => Instruction::NegLong { dest, src },
        NOT_LONG => Instruction::NotLong { dest, src },
        NEG_FLOAT => Instruction::NegFloat { dest, src },
        NEG_DOUBLE => Instruction::NegDouble { dest, src },
        INT_TO_LONG => Instruction::IntToLong { dest, src },
        INT_TO_FLOAT => Instruction::IntToFloat { dest, src },
        INT_TO_DOUBLE => Instruction::IntToDouble { dest, src },
        LONG_TO_INT => Instruction::LongToInt { dest, src },
        LONG_TO_FLOAT => Instruction::LongToFloat { dest, src },
        LONG_TO_DOUBLE => Instruction::LongToDouble { dest, src },
        FLOAT_TO_INT => Instruction::FloatToInt { dest, src },
        FLOAT_TO_LONG => Instruction::FloatToLong { dest, src },
        FLOAT_TO_DOUBLE => Instruction::FloatToDouble { dest, src },
        DOUBLE_TO_INT => Instruction::DoubleToInt { dest, src },
        DOUBLE_TO_LONG => Instruction::DoubleToLong { dest, src },
        DOUBLE_TO_FLOAT => Instruction::DoubleToFloat { dest, src },
        INT_TO_BYTE => Instruction::IntToByte { dest, src },
        INT_TO_CHAR => Instruction::IntToChar { dest, src },
        INT_TO_SHORT => Instruction::IntToShort { dest, src },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_binary(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let [dest, a, b] = decode_23x(buf, unit_off);

    Ok(match u16::from(opcode) {
        ADD_INT => Instruction::AddInt { dest, a, b },
        SUB_INT => Instruction::SubInt { dest, a, b },
        MUL_INT => Instruction::MulInt { dest, a, b },
        DIV_INT => Instruction::DivInt { dest, a, b },
        REM_INT => Instruction::RemInt { dest, a, b },
        AND_INT => Instruction::AndInt { dest, a, b },
        OR_INT => Instruction::OrInt { dest, a, b },
        XOR_INT => Instruction::XorInt { dest, a, b },
        SHL_INT => Instruction::ShlInt { dest, a, b },
        SHR_INT => Instruction::ShrInt { dest, a, b },
        USHR_INT => Instruction::UshrInt { dest, a, b },
        ADD_LONG => Instruction::AddLong { dest, a, b },
        SUB_LONG => Instruction::SubLong { dest, a, b },
        MUL_LONG => Instruction::MulLong { dest, a, b },
        DIV_LONG => Instruction::DivLong { dest, a, b },
        REM_LONG => Instruction::RemLong { dest, a, b },
        AND_LONG => Instruction::AndLong { dest, a, b },
        OR_LONG => Instruction::OrLong { dest, a, b },
        XOR_LONG => Instruction::XorLong { dest, a, b },
        SHL_LONG => Instruction::ShlLong { dest, a, b },
        SHR_LONG => Instruction::ShrLong { dest, a, b },
        USHR_LONG => Instruction::UshrLong { dest, a, b },
        ADD_FLOAT => Instruction::AddFloat { dest, a, b },
        SUB_FLOAT => Instruction::SubFloat { dest, a, b },
        MUL_FLOAT => Instruction::MulFloat { dest, a, b },
        DIV_FLOAT => Instruction::DivFloat { dest, a, b },
        REM_FLOAT => Instruction::RemFloat { dest, a, b },
        ADD_DOUBLE => Instruction::AddDouble { dest, a, b },
        SUB_DOUBLE => Instruction::SubDouble { dest, a, b },
        MUL_DOUBLE => Instruction::MulDouble { dest, a, b },
        DIV_DOUBLE => Instruction::DivDouble { dest, a, b },
        REM_DOUBLE => Instruction::RemDouble { dest, a, b },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_binary_2addr(unit0: u16, opcode: u8) -> Result<Instruction> {
    let (dest_a, b) = nibbles(unit0);

    Ok(match u16::from(opcode) {
        ADD_INT2_ADDR => Instruction::AddInt2Addr { dest_a, b },
        SUB_INT2_ADDR => Instruction::SubInt2Addr { dest_a, b },
        MUL_INT2_ADDR => Instruction::MulInt2Addr { dest_a, b },
        DIV_INT2_ADDR => Instruction::DivInt2Addr { dest_a, b },
        REM_INT2_ADDR => Instruction::RemInt2Addr { dest_a, b },
        AND_INT2_ADDR => Instruction::AndInt2Addr { dest_a, b },
        OR_INT2_ADDR => Instruction::OrInt2Addr { dest_a, b },
        XOR_INT2_ADDR => Instruction::XorInt2Addr { dest_a, b },
        SHL_INT2_ADDR => Instruction::ShlInt2Addr { dest_a, b },
        SHR_INT2_ADDR => Instruction::ShrInt2Addr { dest_a, b },
        USHR_INT2_ADDR => Instruction::UshrInt2Addr { dest_a, b },
        ADD_LONG2_ADDR => Instruction::AddLong2Addr { dest_a, b },
        SUB_LONG2_ADDR => Instruction::SubLong2Addr { dest_a, b },
        MUL_LONG2_ADDR => Instruction::MulLong2Addr { dest_a, b },
        DIV_LONG2_ADDR => Instruction::DivLong2Addr { dest_a, b },
        REM_LONG2_ADDR => Instruction::RemLong2Addr { dest_a, b },
        AND_LONG2_ADDR => Instruction::AndLong2Addr { dest_a, b },
        OR_LONG2_ADDR => Instruction::OrLong2Addr { dest_a, b },
        XOR_LONG2_ADDR => Instruction::XorLong2Addr { dest_a, b },
        SHL_LONG2_ADDR => Instruction::ShlLong2Addr { dest_a, b },
        SHR_LONG2_ADDR => Instruction::ShrLong2Addr { dest_a, b },
        USHR_LONG2_ADDR => Instruction::UshrLong2Addr { dest_a, b },
        ADD_FLOAT2_ADDR => Instruction::AddFloat2Addr { dest_a, b },
        SUB_FLOAT2_ADDR => Instruction::SubFloat2Addr { dest_a, b },
        MUL_FLOAT2_ADDR => Instruction::MulFloat2Addr { dest_a, b },
        DIV_FLOAT2_ADDR => Instruction::DivFloat2Addr { dest_a, b },
        REM_FLOAT2_ADDR => Instruction::RemFloat2Addr { dest_a, b },
        ADD_DOUBLE2_ADDR => Instruction::AddDouble2Addr { dest_a, b },
        SUB_DOUBLE2_ADDR => Instruction::SubDouble2Addr { dest_a, b },
        MUL_DOUBLE2_ADDR => Instruction::MulDouble2Addr { dest_a, b },
        DIV_DOUBLE2_ADDR => Instruction::DivDouble2Addr { dest_a, b },
        REM_DOUBLE2_ADDR => Instruction::RemDouble2Addr { dest_a, b },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_binary_lit16(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let (dest, src) = nibbles(unit0);
    let literal = u16_at(buf, unit_off + 2) as i16;

    Ok(match u16::from(opcode) {
        ADD_INT_LIT16 => Instruction::AddIntLit16 { dest, src, literal },
        RSUB_INT_LIT16 => Instruction::RsubIntLit16 { dest, src, literal },
        MUL_INT_LIT16 => Instruction::MulIntLit16 { dest, src, literal },
        DIV_INT_LIT16 => Instruction::DivIntLit16 { dest, src, literal },
        REM_INT_LIT16 => Instruction::RemIntLit16 { dest, src, literal },
        AND_INT_LIT16 => Instruction::AndIntLit16 { dest, src, literal },
        OR_INT_LIT16 => Instruction::OrIntLit16 { dest, src, literal },
        XOR_INT_LIT16 => Instruction::XorIntLit16 { dest, src, literal },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}

fn decode_binary_lit8(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let dest = hi8(unit0);
    let packed = u16_at(buf, unit_off + 2);
    let src = packed as u8;
    let literal = (packed >> 8) as i8;

    Ok(match u16::from(opcode) {
        ADD_INT_LIT8 => Instruction::AddIntLit8 { dest, src, literal },
        RSUB_INT_LIT8 => Instruction::RsubIntLit8 { dest, src, literal },
        MUL_INT_LIT8 => Instruction::MulIntLit8 { dest, src, literal },
        DIV_INT_LIT8 => Instruction::DivIntLit8 { dest, src, literal },
        REM_INT_LIT8 => Instruction::RemIntLit8 { dest, src, literal },
        AND_INT_LIT8 => Instruction::AndIntLit8 { dest, src, literal },
        OR_INT_LIT8 => Instruction::OrIntLit8 { dest, src, literal },
        XOR_INT_LIT8 => Instruction::XorIntLit8 { dest, src, literal },
        SHL_INT_LIT8 => Instruction::ShlIntLit8 { dest, src, literal },
        SHR_INT_LIT8 => Instruction::ShrIntLit8 { dest, src, literal },
        USHR_INT_LIT8 => Instruction::UshrIntLit8 { dest, src, literal },
        _ => {
            return Err(invalid(
                "instruction opcode",
                format!("{opcode:#x} is outside the decoded family"),
            ));
        }
    })
}
