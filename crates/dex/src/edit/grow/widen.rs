// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::allocation::Allocation;
use crate::instruction::Instruction;
use crate::instruction::operands::RegisterKind;

/// The encoding of `insn` that reaches its operands once they shift.
pub(super) fn widen(insn: &Instruction, allocation: &Allocation<'_>) -> Instruction {
    use Instruction::{
        Const4, Const16, Move, Move16, MoveFrom16, MoveObject, MoveObject16, MoveObjectFrom16,
        MoveWide, MoveWide16, MoveWideFrom16,
    };
    macro_rules! widen_binary {
        ($($compact:ident => $wide:ident),* $(,)?) => {
            match *insn {
                Move { dest, src } => Move16 {
                    dest: u16::from(dest),
                    src: u16::from(src),
                },
                MoveFrom16 { dest, src } => Move16 {
                    dest: u16::from(dest),
                    src,
                },
                MoveWide { dest, src } => MoveWide16 {
                    dest: u16::from(dest),
                    src: u16::from(src),
                },
                MoveWideFrom16 { dest, src } => MoveWide16 {
                    dest: u16::from(dest),
                    src,
                },
                MoveObject { dest, src } => MoveObject16 {
                    dest: u16::from(dest),
                    src: u16::from(src),
                },
                MoveObjectFrom16 { dest, src } => MoveObject16 {
                    dest: u16::from(dest),
                    src,
                },
                Const4 { dest, value } if allocation.shift(u16::from(dest)) > 15 => Const16 {
                    dest,
                    value: i16::from(value),
                },
                $(Instruction::$compact { dest_a, b }
                    if allocation.shift(u16::from(dest_a)) > 15
                        || allocation.shift(u16::from(b)) > 15 =>
                {
                    Instruction::$wide { dest: dest_a, a: dest_a, b }
                },)*
                _ => insn.clone(),
            }
        };
    }
    widen_binary!(
        AddInt2Addr => AddInt,
        SubInt2Addr => SubInt,
        MulInt2Addr => MulInt,
        DivInt2Addr => DivInt,
        RemInt2Addr => RemInt,
        AndInt2Addr => AndInt,
        OrInt2Addr => OrInt,
        XorInt2Addr => XorInt,
        ShlInt2Addr => ShlInt,
        ShrInt2Addr => ShrInt,
        UshrInt2Addr => UshrInt,
        AddLong2Addr => AddLong,
        SubLong2Addr => SubLong,
        MulLong2Addr => MulLong,
        DivLong2Addr => DivLong,
        RemLong2Addr => RemLong,
        AndLong2Addr => AndLong,
        OrLong2Addr => OrLong,
        XorLong2Addr => XorLong,
        ShlLong2Addr => ShlLong,
        ShrLong2Addr => ShrLong,
        UshrLong2Addr => UshrLong,
        AddFloat2Addr => AddFloat,
        SubFloat2Addr => SubFloat,
        MulFloat2Addr => MulFloat,
        DivFloat2Addr => DivFloat,
        RemFloat2Addr => RemFloat,
        AddDouble2Addr => AddDouble,
        SubDouble2Addr => SubDouble,
        MulDouble2Addr => MulDouble,
        DivDouble2Addr => DivDouble,
        RemDouble2Addr => RemDouble,
    )
}

/// The smallest encoding of a 16-bit move, which widening and staging can leave behind.
pub(super) fn narrowed(instruction: Instruction) -> Instruction {
    match instruction {
        Instruction::Move16 { dest, src } => move_register(dest, src, RegisterKind::Value),
        Instruction::MoveObject16 { dest, src } => move_register(dest, src, RegisterKind::Object),
        Instruction::MoveWide16 { dest, src } => move_register(dest, src, RegisterKind::Wide),
        instruction => instruction,
    }
}

pub(super) fn move_register(dest: u16, src: u16, kind: RegisterKind) -> Instruction {
    use Instruction::{
        Move, Move16, MoveFrom16, MoveObject, MoveObject16, MoveObjectFrom16, MoveWide, MoveWide16,
        MoveWideFrom16,
    };
    match kind {
        RegisterKind::Wide if dest <= 15 && src <= 15 => MoveWide {
            dest: dest as u8,
            src: src as u8,
        },
        RegisterKind::Wide if dest <= 255 => MoveWideFrom16 {
            dest: dest as u8,
            src,
        },
        RegisterKind::Wide => MoveWide16 { dest, src },
        RegisterKind::Object if dest <= 15 && src <= 15 => MoveObject {
            dest: dest as u8,
            src: src as u8,
        },
        RegisterKind::Object if dest <= 255 => MoveObjectFrom16 {
            dest: dest as u8,
            src,
        },
        RegisterKind::Object => MoveObject16 { dest, src },
        RegisterKind::Value if dest <= 15 && src <= 15 => Move {
            dest: dest as u8,
            src: src as u8,
        },
        RegisterKind::Value if dest <= 255 => MoveFrom16 {
            dest: dest as u8,
            src,
        },
        RegisterKind::Value => Move16 { dest, src },
    }
}
