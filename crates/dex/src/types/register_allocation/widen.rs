// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Allocation, Instruction};

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
