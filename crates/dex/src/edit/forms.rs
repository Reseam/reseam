// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The encodings of one operation that differ in how far their operands reach.

use crate::instruction::Instruction;
use crate::instruction::operands::RegisterKind;

/// The form of a move or constant that reaches every register its operation can name, for an
/// instruction written before its registers are chosen; [`narrowed`] shrinks it again after.
pub(super) fn widest(instruction: Instruction) -> Instruction {
    use Instruction::{
        Const, Const4, Const16, ConstHigh16, ConstWide, ConstWide16, ConstWide32, ConstWideHigh16,
        Move, Move16, MoveFrom16, MoveObject, MoveObject16, MoveObjectFrom16, MoveWide, MoveWide16,
        MoveWideFrom16,
    };
    let literal = instruction.literal();
    match instruction {
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
        Const4 { dest, .. } | Const16 { dest, .. } | ConstHigh16 { dest, .. } => Const {
            dest,
            value: literal.expect("a constant loads a literal") as i32,
        },
        ConstWide16 { dest, .. } | ConstWide32 { dest, .. } | ConstWideHigh16 { dest, .. } => {
            ConstWide {
                dest,
                value: literal.expect("a constant loads a literal"),
            }
        }
        instruction => instruction,
    }
}

/// The smallest form of a move or constant for the registers and value it names.
pub(super) fn narrowed(instruction: Instruction) -> Instruction {
    match instruction {
        Instruction::Const { dest, value } => constant(dest, value),
        Instruction::ConstWide { dest, value } => wide_constant(dest, value),
        instruction => narrowed_move(instruction),
    }
}

/// The smallest encoding of a 16-bit move, which widening and staging can leave behind.
pub(super) fn narrowed_move(instruction: Instruction) -> Instruction {
    match instruction {
        Instruction::Move16 { dest, src } => move_register(dest, src, RegisterKind::Value),
        Instruction::MoveObject16 { dest, src } => move_register(dest, src, RegisterKind::Object),
        Instruction::MoveWide16 { dest, src } => move_register(dest, src, RegisterKind::Wide),
        instruction => instruction,
    }
}

fn constant(dest: u8, value: i32) -> Instruction {
    if dest <= 15
        && let Ok(value) = i8::try_from(value)
        && (-8..=7).contains(&value)
    {
        Instruction::Const4 { dest, value }
    } else if let Ok(value) = i16::try_from(value) {
        Instruction::Const16 { dest, value }
    } else if value.trailing_zeros() >= 16 {
        Instruction::ConstHigh16 {
            dest,
            value: (value >> 16) as i16,
        }
    } else {
        Instruction::Const { dest, value }
    }
}

fn wide_constant(dest: u8, value: i64) -> Instruction {
    if let Ok(value) = i16::try_from(value) {
        Instruction::ConstWide16 { dest, value }
    } else if let Ok(value) = i32::try_from(value) {
        Instruction::ConstWide32 { dest, value }
    } else if value.trailing_zeros() >= 48 {
        Instruction::ConstWideHigh16 {
            dest,
            value: (value >> 48) as i16,
        }
    } else {
        Instruction::ConstWide { dest, value }
    }
}

pub fn move_register(dest: u16, src: u16, kind: RegisterKind) -> Instruction {
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
