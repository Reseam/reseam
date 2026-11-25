// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 32-bit branches, calls and miscellaneous control.

use super::fields::branch_offset;
use crate::arm::instruction::{
    Effect, Flow, Instruction, bit, field, register, register_at, sign_extend, word_aligned,
};
use crate::arm::mode::Mode;

/// The offset of a conditional B.W: S:J2:J1:imm6:imm11:0.
fn conditional_offset(first: u32, second: u32) -> i64 {
    let value = field(first, 10, 1) << 20
        | field(second, 11, 1) << 19
        | field(second, 13, 1) << 18
        | field(first, 0, 6) << 12
        | field(second, 0, 11) << 1;
    sign_extend(value, 21)
}

/// Branches by hw2 bits 14 and 12; the conditional ones share their space with miscellaneous
/// control, which a condition of 0b111x marks.
pub(super) fn branch_or_control(first: u32, second: u32, pc: u64) -> Instruction {
    match (bit(second, 14), bit(second, 12)) {
        (false, false) if field(first, 7, 3) != 0b111 => Instruction::from(Flow::Branch(
            pc.wrapping_add_signed(conditional_offset(first, second)),
        ))
        .conditional(),
        (false, false) => match field(first, 4, 7) {
            // BXJ, and SUBS PC, LR returning from an exception.
            0b011_1100 | 0b011_1101 => Flow::Stop.into(),
            // MRS.
            0b011_1110 | 0b011_1111 => Effect::Writes(register(register_at(second, 8))).into(),
            // UDF.
            0b111_1111 if field(second, 12, 3) == 0b010 => Flow::Stop.into(),
            _ => Effect::None.into(),
        },
        (false, true) => Flow::Branch(pc.wrapping_add_signed(branch_offset(first, second))).into(),
        (true, false) => Flow::Call(
            word_aligned(pc).wrapping_add_signed(branch_offset(first, second)),
            Mode::Arm,
        )
        .into(),
        (true, true) => Flow::Call(
            pc.wrapping_add_signed(branch_offset(first, second)),
            Mode::Thumb,
        )
        .into(),
    }
}
