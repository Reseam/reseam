// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::super::arithmetic::decode_23x;
use super::super::format::u16_at;
use super::{hi8, nibbles, DecodedInstruction};

pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<DecodedInstruction> {
    let unit0 = u16_at(buf, unit_off);
    let decoded = match opcode {
        0x7b..=0x8f => decode_unary(unit0, opcode),
        0x90..=0xaf => decode_binary(buf, unit_off, opcode),
        0xb0..=0xcf => decode_binary_2addr(unit0, opcode),
        0xd0..=0xd7 => decode_binary_lit16(unit0, buf, unit_off, opcode),
        0xd8..=0xe2 => decode_binary_lit8(unit0, buf, unit_off, opcode),
        _ => unreachable!(),
    };
    Ok(decoded)
}

fn decode_unary(unit0: u16, opcode: u8) -> DecodedInstruction {
    let (dest, src) = nibbles(unit0);
    let instruction = match opcode {
        0x7b => Instruction::NegInt { dest, src },
        0x7c => Instruction::NotInt { dest, src },
        0x7d => Instruction::NegLong { dest, src },
        0x7e => Instruction::NotLong { dest, src },
        0x7f => Instruction::NegFloat { dest, src },
        0x80 => Instruction::NegDouble { dest, src },
        0x81 => Instruction::IntToLong { dest, src },
        0x82 => Instruction::IntToFloat { dest, src },
        0x83 => Instruction::IntToDouble { dest, src },
        0x84 => Instruction::LongToInt { dest, src },
        0x85 => Instruction::LongToFloat { dest, src },
        0x86 => Instruction::LongToDouble { dest, src },
        0x87 => Instruction::FloatToInt { dest, src },
        0x88 => Instruction::FloatToLong { dest, src },
        0x89 => Instruction::FloatToDouble { dest, src },
        0x8a => Instruction::DoubleToInt { dest, src },
        0x8b => Instruction::DoubleToLong { dest, src },
        0x8c => Instruction::DoubleToFloat { dest, src },
        0x8d => Instruction::IntToByte { dest, src },
        0x8e => Instruction::IntToChar { dest, src },
        0x8f => Instruction::IntToShort { dest, src },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 1)
}

fn decode_binary(buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let (dest, a, b) = decode_23x(buf, unit_off);
    let instruction = match opcode {
        0x90 => Instruction::AddInt { dest, a, b },
        0x91 => Instruction::SubInt { dest, a, b },
        0x92 => Instruction::MulInt { dest, a, b },
        0x93 => Instruction::DivInt { dest, a, b },
        0x94 => Instruction::RemInt { dest, a, b },
        0x95 => Instruction::AndInt { dest, a, b },
        0x96 => Instruction::OrInt { dest, a, b },
        0x97 => Instruction::XorInt { dest, a, b },
        0x98 => Instruction::ShlInt { dest, a, b },
        0x99 => Instruction::ShrInt { dest, a, b },
        0x9a => Instruction::UshrInt { dest, a, b },
        0x9b => Instruction::AddLong { dest, a, b },
        0x9c => Instruction::SubLong { dest, a, b },
        0x9d => Instruction::MulLong { dest, a, b },
        0x9e => Instruction::DivLong { dest, a, b },
        0x9f => Instruction::RemLong { dest, a, b },
        0xa0 => Instruction::AndLong { dest, a, b },
        0xa1 => Instruction::OrLong { dest, a, b },
        0xa2 => Instruction::XorLong { dest, a, b },
        0xa3 => Instruction::ShlLong { dest, a, b },
        0xa4 => Instruction::ShrLong { dest, a, b },
        0xa5 => Instruction::UshrLong { dest, a, b },
        0xa6 => Instruction::AddFloat { dest, a, b },
        0xa7 => Instruction::SubFloat { dest, a, b },
        0xa8 => Instruction::MulFloat { dest, a, b },
        0xa9 => Instruction::DivFloat { dest, a, b },
        0xaa => Instruction::RemFloat { dest, a, b },
        0xab => Instruction::AddDouble { dest, a, b },
        0xac => Instruction::SubDouble { dest, a, b },
        0xad => Instruction::MulDouble { dest, a, b },
        0xae => Instruction::DivDouble { dest, a, b },
        0xaf => Instruction::RemDouble { dest, a, b },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_binary_2addr(unit0: u16, opcode: u8) -> DecodedInstruction {
    let (dest_a, b) = nibbles(unit0);
    let instruction = match opcode {
        0xb0 => Instruction::AddInt2Addr { dest_a, b },
        0xb1 => Instruction::SubInt2Addr { dest_a, b },
        0xb2 => Instruction::MulInt2Addr { dest_a, b },
        0xb3 => Instruction::DivInt2Addr { dest_a, b },
        0xb4 => Instruction::RemInt2Addr { dest_a, b },
        0xb5 => Instruction::AndInt2Addr { dest_a, b },
        0xb6 => Instruction::OrInt2Addr { dest_a, b },
        0xb7 => Instruction::XorInt2Addr { dest_a, b },
        0xb8 => Instruction::ShlInt2Addr { dest_a, b },
        0xb9 => Instruction::ShrInt2Addr { dest_a, b },
        0xba => Instruction::UshrInt2Addr { dest_a, b },
        0xbb => Instruction::AddLong2Addr { dest_a, b },
        0xbc => Instruction::SubLong2Addr { dest_a, b },
        0xbd => Instruction::MulLong2Addr { dest_a, b },
        0xbe => Instruction::DivLong2Addr { dest_a, b },
        0xbf => Instruction::RemLong2Addr { dest_a, b },
        0xc0 => Instruction::AndLong2Addr { dest_a, b },
        0xc1 => Instruction::OrLong2Addr { dest_a, b },
        0xc2 => Instruction::XorLong2Addr { dest_a, b },
        0xc3 => Instruction::ShlLong2Addr { dest_a, b },
        0xc4 => Instruction::ShrLong2Addr { dest_a, b },
        0xc5 => Instruction::UshrLong2Addr { dest_a, b },
        0xc6 => Instruction::AddFloat2Addr { dest_a, b },
        0xc7 => Instruction::SubFloat2Addr { dest_a, b },
        0xc8 => Instruction::MulFloat2Addr { dest_a, b },
        0xc9 => Instruction::DivFloat2Addr { dest_a, b },
        0xca => Instruction::RemFloat2Addr { dest_a, b },
        0xcb => Instruction::AddDouble2Addr { dest_a, b },
        0xcc => Instruction::SubDouble2Addr { dest_a, b },
        0xcd => Instruction::MulDouble2Addr { dest_a, b },
        0xce => Instruction::DivDouble2Addr { dest_a, b },
        0xcf => Instruction::RemDouble2Addr { dest_a, b },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 1)
}

fn decode_binary_lit16(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let (dest, src) = nibbles(unit0);
    let literal = u16_at(buf, unit_off + 2) as i16;
    let instruction = match opcode {
        0xd0 => Instruction::AddIntLit16 { dest, src, literal },
        0xd1 => Instruction::RsubIntLit16 { dest, src, literal },
        0xd2 => Instruction::MulIntLit16 { dest, src, literal },
        0xd3 => Instruction::DivIntLit16 { dest, src, literal },
        0xd4 => Instruction::RemIntLit16 { dest, src, literal },
        0xd5 => Instruction::AndIntLit16 { dest, src, literal },
        0xd6 => Instruction::OrIntLit16 { dest, src, literal },
        0xd7 => Instruction::XorIntLit16 { dest, src, literal },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_binary_lit8(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let dest = hi8(unit0);
    let packed = u16_at(buf, unit_off + 2);
    let src = packed as u8;
    let literal = (packed >> 8) as i8;

    let instruction = match opcode {
        0xd8 => Instruction::AddIntLit8 { dest, src, literal },
        0xd9 => Instruction::RsubIntLit8 { dest, src, literal },
        0xda => Instruction::MulIntLit8 { dest, src, literal },
        0xdb => Instruction::DivIntLit8 { dest, src, literal },
        0xdc => Instruction::RemIntLit8 { dest, src, literal },
        0xdd => Instruction::AndIntLit8 { dest, src, literal },
        0xde => Instruction::OrIntLit8 { dest, src, literal },
        0xdf => Instruction::XorIntLit8 { dest, src, literal },
        0xe0 => Instruction::ShlIntLit8 { dest, src, literal },
        0xe1 => Instruction::ShrIntLit8 { dest, src, literal },
        0xe2 => Instruction::UshrIntLit8 { dest, src, literal },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}
