// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

use super::{encode_23x, pack_12x, pack_aa_op};

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::CmpLFloat { dest, a, b } => encode_23x(code, 0x2d, *dest, *a, *b),
        Instruction::CmpGFloat { dest, a, b } => encode_23x(code, 0x2e, *dest, *a, *b),
        Instruction::CmpLDouble { dest, a, b } => encode_23x(code, 0x2f, *dest, *a, *b),
        Instruction::CmpGDouble { dest, a, b } => encode_23x(code, 0x30, *dest, *a, *b),
        Instruction::CmpLong { dest, a, b } => encode_23x(code, 0x31, *dest, *a, *b),

        Instruction::NegInt { dest, src } => code.push(pack_12x(0x7b, *dest, *src)?),
        Instruction::NotInt { dest, src } => code.push(pack_12x(0x7c, *dest, *src)?),
        Instruction::NegLong { dest, src } => code.push(pack_12x(0x7d, *dest, *src)?),
        Instruction::NotLong { dest, src } => code.push(pack_12x(0x7e, *dest, *src)?),
        Instruction::NegFloat { dest, src } => code.push(pack_12x(0x7f, *dest, *src)?),
        Instruction::NegDouble { dest, src } => code.push(pack_12x(0x80, *dest, *src)?),
        Instruction::IntToLong { dest, src } => code.push(pack_12x(0x81, *dest, *src)?),
        Instruction::IntToFloat { dest, src } => code.push(pack_12x(0x82, *dest, *src)?),
        Instruction::IntToDouble { dest, src } => code.push(pack_12x(0x83, *dest, *src)?),
        Instruction::LongToInt { dest, src } => code.push(pack_12x(0x84, *dest, *src)?),
        Instruction::LongToFloat { dest, src } => code.push(pack_12x(0x85, *dest, *src)?),
        Instruction::LongToDouble { dest, src } => code.push(pack_12x(0x86, *dest, *src)?),
        Instruction::FloatToInt { dest, src } => code.push(pack_12x(0x87, *dest, *src)?),
        Instruction::FloatToLong { dest, src } => code.push(pack_12x(0x88, *dest, *src)?),
        Instruction::FloatToDouble { dest, src } => code.push(pack_12x(0x89, *dest, *src)?),
        Instruction::DoubleToInt { dest, src } => code.push(pack_12x(0x8a, *dest, *src)?),
        Instruction::DoubleToLong { dest, src } => code.push(pack_12x(0x8b, *dest, *src)?),
        Instruction::DoubleToFloat { dest, src } => code.push(pack_12x(0x8c, *dest, *src)?),
        Instruction::IntToByte { dest, src } => code.push(pack_12x(0x8d, *dest, *src)?),
        Instruction::IntToChar { dest, src } => code.push(pack_12x(0x8e, *dest, *src)?),
        Instruction::IntToShort { dest, src } => code.push(pack_12x(0x8f, *dest, *src)?),

        Instruction::AddInt { dest, a, b } => encode_23x(code, 0x90, *dest, *a, *b),
        Instruction::SubInt { dest, a, b } => encode_23x(code, 0x91, *dest, *a, *b),
        Instruction::MulInt { dest, a, b } => encode_23x(code, 0x92, *dest, *a, *b),
        Instruction::DivInt { dest, a, b } => encode_23x(code, 0x93, *dest, *a, *b),
        Instruction::RemInt { dest, a, b } => encode_23x(code, 0x94, *dest, *a, *b),
        Instruction::AndInt { dest, a, b } => encode_23x(code, 0x95, *dest, *a, *b),
        Instruction::OrInt { dest, a, b } => encode_23x(code, 0x96, *dest, *a, *b),
        Instruction::XorInt { dest, a, b } => encode_23x(code, 0x97, *dest, *a, *b),
        Instruction::ShlInt { dest, a, b } => encode_23x(code, 0x98, *dest, *a, *b),
        Instruction::ShrInt { dest, a, b } => encode_23x(code, 0x99, *dest, *a, *b),
        Instruction::UshrInt { dest, a, b } => encode_23x(code, 0x9a, *dest, *a, *b),
        Instruction::AddLong { dest, a, b } => encode_23x(code, 0x9b, *dest, *a, *b),
        Instruction::SubLong { dest, a, b } => encode_23x(code, 0x9c, *dest, *a, *b),
        Instruction::MulLong { dest, a, b } => encode_23x(code, 0x9d, *dest, *a, *b),
        Instruction::DivLong { dest, a, b } => encode_23x(code, 0x9e, *dest, *a, *b),
        Instruction::RemLong { dest, a, b } => encode_23x(code, 0x9f, *dest, *a, *b),
        Instruction::AndLong { dest, a, b } => encode_23x(code, 0xa0, *dest, *a, *b),
        Instruction::OrLong { dest, a, b } => encode_23x(code, 0xa1, *dest, *a, *b),
        Instruction::XorLong { dest, a, b } => encode_23x(code, 0xa2, *dest, *a, *b),
        Instruction::ShlLong { dest, a, b } => encode_23x(code, 0xa3, *dest, *a, *b),
        Instruction::ShrLong { dest, a, b } => encode_23x(code, 0xa4, *dest, *a, *b),
        Instruction::UshrLong { dest, a, b } => encode_23x(code, 0xa5, *dest, *a, *b),
        Instruction::AddFloat { dest, a, b } => encode_23x(code, 0xa6, *dest, *a, *b),
        Instruction::SubFloat { dest, a, b } => encode_23x(code, 0xa7, *dest, *a, *b),
        Instruction::MulFloat { dest, a, b } => encode_23x(code, 0xa8, *dest, *a, *b),
        Instruction::DivFloat { dest, a, b } => encode_23x(code, 0xa9, *dest, *a, *b),
        Instruction::RemFloat { dest, a, b } => encode_23x(code, 0xaa, *dest, *a, *b),
        Instruction::AddDouble { dest, a, b } => encode_23x(code, 0xab, *dest, *a, *b),
        Instruction::SubDouble { dest, a, b } => encode_23x(code, 0xac, *dest, *a, *b),
        Instruction::MulDouble { dest, a, b } => encode_23x(code, 0xad, *dest, *a, *b),
        Instruction::DivDouble { dest, a, b } => encode_23x(code, 0xae, *dest, *a, *b),
        Instruction::RemDouble { dest, a, b } => encode_23x(code, 0xaf, *dest, *a, *b),

        Instruction::AddInt2Addr { dest_a, b } => code.push(pack_12x(0xb0, *dest_a, *b)?),
        Instruction::SubInt2Addr { dest_a, b } => code.push(pack_12x(0xb1, *dest_a, *b)?),
        Instruction::MulInt2Addr { dest_a, b } => code.push(pack_12x(0xb2, *dest_a, *b)?),
        Instruction::DivInt2Addr { dest_a, b } => code.push(pack_12x(0xb3, *dest_a, *b)?),
        Instruction::RemInt2Addr { dest_a, b } => code.push(pack_12x(0xb4, *dest_a, *b)?),
        Instruction::AndInt2Addr { dest_a, b } => code.push(pack_12x(0xb5, *dest_a, *b)?),
        Instruction::OrInt2Addr { dest_a, b } => code.push(pack_12x(0xb6, *dest_a, *b)?),
        Instruction::XorInt2Addr { dest_a, b } => code.push(pack_12x(0xb7, *dest_a, *b)?),
        Instruction::ShlInt2Addr { dest_a, b } => code.push(pack_12x(0xb8, *dest_a, *b)?),
        Instruction::ShrInt2Addr { dest_a, b } => code.push(pack_12x(0xb9, *dest_a, *b)?),
        Instruction::UshrInt2Addr { dest_a, b } => code.push(pack_12x(0xba, *dest_a, *b)?),
        Instruction::AddLong2Addr { dest_a, b } => code.push(pack_12x(0xbb, *dest_a, *b)?),
        Instruction::SubLong2Addr { dest_a, b } => code.push(pack_12x(0xbc, *dest_a, *b)?),
        Instruction::MulLong2Addr { dest_a, b } => code.push(pack_12x(0xbd, *dest_a, *b)?),
        Instruction::DivLong2Addr { dest_a, b } => code.push(pack_12x(0xbe, *dest_a, *b)?),
        Instruction::RemLong2Addr { dest_a, b } => code.push(pack_12x(0xbf, *dest_a, *b)?),
        Instruction::AndLong2Addr { dest_a, b } => code.push(pack_12x(0xc0, *dest_a, *b)?),
        Instruction::OrLong2Addr { dest_a, b } => code.push(pack_12x(0xc1, *dest_a, *b)?),
        Instruction::XorLong2Addr { dest_a, b } => code.push(pack_12x(0xc2, *dest_a, *b)?),
        Instruction::ShlLong2Addr { dest_a, b } => code.push(pack_12x(0xc3, *dest_a, *b)?),
        Instruction::ShrLong2Addr { dest_a, b } => code.push(pack_12x(0xc4, *dest_a, *b)?),
        Instruction::UshrLong2Addr { dest_a, b } => code.push(pack_12x(0xc5, *dest_a, *b)?),
        Instruction::AddFloat2Addr { dest_a, b } => code.push(pack_12x(0xc6, *dest_a, *b)?),
        Instruction::SubFloat2Addr { dest_a, b } => code.push(pack_12x(0xc7, *dest_a, *b)?),
        Instruction::MulFloat2Addr { dest_a, b } => code.push(pack_12x(0xc8, *dest_a, *b)?),
        Instruction::DivFloat2Addr { dest_a, b } => code.push(pack_12x(0xc9, *dest_a, *b)?),
        Instruction::RemFloat2Addr { dest_a, b } => code.push(pack_12x(0xca, *dest_a, *b)?),
        Instruction::AddDouble2Addr { dest_a, b } => code.push(pack_12x(0xcb, *dest_a, *b)?),
        Instruction::SubDouble2Addr { dest_a, b } => code.push(pack_12x(0xcc, *dest_a, *b)?),
        Instruction::MulDouble2Addr { dest_a, b } => code.push(pack_12x(0xcd, *dest_a, *b)?),
        Instruction::DivDouble2Addr { dest_a, b } => code.push(pack_12x(0xce, *dest_a, *b)?),
        Instruction::RemDouble2Addr { dest_a, b } => code.push(pack_12x(0xcf, *dest_a, *b)?),

        Instruction::AddIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd0, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::RsubIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd1, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::MulIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd2, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::DivIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd3, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::RemIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd4, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::AndIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd5, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::OrIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd6, *dest, *src)?);
            code.push(*literal as u16);
        }
        Instruction::XorIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd7, *dest, *src)?);
            code.push(*literal as u16);
        }

        Instruction::AddIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xd8, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::RsubIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xd9, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::MulIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xda, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::DivIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdb, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::RemIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdc, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::AndIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdd, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::OrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xde, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::XorIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdf, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::ShlIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe0, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::ShrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe1, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::UshrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe2, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }

        _ => unreachable!(),
    }
    Ok(())
}
