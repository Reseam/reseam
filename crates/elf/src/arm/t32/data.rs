// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 32-bit data processing, with a shifted register or an immediate.

use super::fields::{Second, immediate12, wide_immediate};
use crate::arm::instruction::{Effect, Flow, Instruction, PC, field, register};

/// The opcodes of TST, TEQ, CMN and CMP, which name the program counter as Rd and only set
/// flags.
fn is_compare(opcode: u32) -> bool {
    matches!(opcode, 0b0000 | 0b0100 | 0b1000 | 0b1101)
}

pub(super) fn shifted_register(first: u32, second: Second) -> Instruction {
    match field(first, 5, 4) {
        opcode if second.rd == PC && is_compare(opcode) => Effect::None.into(),
        _ if second.rd == PC => Flow::Stop.into(),
        _ => Effect::Writes(register(second.rd)).into(),
    }
}

/// `ThumbExpandImm`: an 8-bit value repeated across the word, or a 7-bit one rotated.
fn expand_immediate(first: u32, second: u32) -> u32 {
    let value = immediate12(first, second);
    let byte = field(value, 0, 8);
    match field(value, 8, 4) {
        0b0000 => byte,
        0b0001 => byte << 16 | byte,
        0b0010 => byte << 24 | byte << 8,
        0b0011 => byte * 0x0101_0101,
        _ => (0x80 | field(value, 0, 7)).rotate_right(field(value, 7, 5)),
    }
}

pub(super) fn modified_immediate(first: u32, second: Second, rn: u8) -> Instruction {
    let value = expand_immediate(first, second.halfword);
    let rd = second.rd;
    match field(first, 5, 4) {
        opcode if rd == PC && is_compare(opcode) => Effect::None,
        0b1000 => Effect::Add {
            register: rd,
            base: rn,
            offset: value,
        },
        0b1101 => Effect::Add {
            register: rd,
            base: rn,
            offset: value.wrapping_neg(),
        },
        // MOV and MVN name the program counter as Rn.
        0b0010 if rn == PC => Effect::Constant {
            register: rd,
            value,
        },
        0b0011 if rn == PC => Effect::Constant {
            register: rd,
            value: !value,
        },
        _ => Effect::Writes(register(rd)),
    }
    .into()
}

pub(super) fn plain_immediate(first: u32, second: Second, rn: u8) -> Instruction {
    let rd = second.rd;
    let low = immediate12(first, second.halfword);
    match field(first, 4, 5) {
        0b00000 | 0b01010 if rn == PC => Effect::Writes(register(rd)),
        0b00000 => Effect::Add {
            register: rd,
            base: rn,
            offset: low,
        },
        0b01010 => Effect::Add {
            register: rd,
            base: rn,
            offset: low.wrapping_neg(),
        },
        0b00100 => Effect::Constant {
            register: rd,
            value: wide_immediate(first, second.halfword),
        },
        0b01100 => Effect::High {
            register: rd,
            value: wide_immediate(first, second.halfword),
        },
        _ => Effect::Writes(register(rd)),
    }
    .into()
}
