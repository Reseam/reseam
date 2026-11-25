// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A32 instructions, by the encoding groups of the ARMv7-A architecture manual. Reading the
//! program counter gives the instruction's address plus 8.

mod data;
mod emit;
mod fields;
mod memory;

pub(super) use emit::{jump, nop, return_code};

use data::{data_and_miscellaneous, media};
use fields::{branch_offset, rd, rn};
use memory::{block_transfer, coprocessor, load_store};

use super::instruction::{Effect, Flow, Instruction, bit, field, register};
use super::mode::Mode;

const ALWAYS: u32 = 0b1110;
const UNCONDITIONAL: u32 = 0b1111;

pub(super) fn decode(word: u32, address: u64) -> Instruction {
    let pc = address + 8;
    let condition = word >> 28;
    if condition == UNCONDITIONAL {
        return unconditional(word, pc);
    }
    let instruction = match field(word, 25, 3) {
        0b000 | 0b001 => data_and_miscellaneous(word, pc),
        0b010 => load_store(word, pc),
        0b011 if !bit(word, 4) => load_store(word, pc),
        0b011 => media(word),
        0b100 => block_transfer(word),
        0b101 if bit(word, 24) => Flow::Call(branch_target(word, pc), Mode::Arm).into(),
        0b101 => Flow::Branch(branch_target(word, pc)).into(),
        _ => coprocessor(word).into(),
    };
    if condition == ALWAYS {
        instruction
    } else {
        instruction.conditional()
    }
}

/// The target of B, BL and BLX: a signed 24-bit offset in words.
fn branch_target(word: u32, pc: u64) -> u64 {
    pc.wrapping_add_signed(branch_offset(word))
}

/// The instructions that use the condition field's last value: BLX to Thumb, SIMD, barriers
/// and hints.
fn unconditional(word: u32, pc: u64) -> Instruction {
    match field(word, 25, 3) {
        // BLX's H bit adds a halfword, since Thumb code is halfword aligned.
        0b101 => {
            let half = if bit(word, 24) { 2 } else { 0 };
            Flow::Call(branch_target(word, pc) + half, Mode::Thumb).into()
        }
        // Element and structure loads and stores write their base back when Rm is not the
        // program counter.
        0b010 if !bit(word, 24) && !bit(word, 20) && word & 0xf != 0xf => {
            Effect::Writes(register(rn(word))).into()
        }
        0b100 if bit(word, 20) => Flow::Stop.into(),
        0b111 if bit(word, 4) && bit(word, 20) => Effect::Writes(register(rd(word))).into(),
        _ => Effect::None.into(),
    }
}
