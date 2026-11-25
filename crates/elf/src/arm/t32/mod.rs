// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Thumb instructions, 16 or 32 bits, by the encoding groups of the ARMv7-A architecture manual.
//! Reading the program counter gives the instruction's address plus 4.

mod branch;
mod data;
mod emit;
mod fields;
mod memory;
mod narrow;

pub(super) use emit::{jump, nop, return_code};
pub(super) use narrow::decode_narrow;

use branch::branch_or_control;
use data::{modified_immediate, plain_immediate, shifted_register};
use fields::Second;
use memory::{coprocessor, dual_exclusive_table, load_single, load_store_multiple, store_single};

use super::instruction::{Effect, Flow, Instruction, bit, field, register};

/// Whether `first` starts a 32-bit instruction: its top five bits are 0b11101, 0b11110 or
/// 0b11111.
pub(super) fn is_wide(first: u32) -> bool {
    first >> 11 >= 0b11101
}

pub(super) fn decode_wide(first: u32, second: u32, address: u64) -> Instruction {
    let pc = address + 4;
    let op2 = field(first, 4, 7);
    let rn = field(first, 0, 4) as u8;
    let second = Second::new(second);
    match field(first, 11, 2) {
        0b01 if op2 & 0b110_0100 == 0 => load_store_multiple(first, second, rn),
        0b01 if op2 & 0b110_0100 == 0b000_0100 => dual_exclusive_table(first, second, rn, pc),
        0b01 if op2 & 0b110_0000 == 0b010_0000 => shifted_register(first, second),
        0b10 if bit(second.halfword, 15) => branch_or_control(first, second.halfword, pc),
        0b10 if bit(first, 9) => plain_immediate(first, second, rn),
        0b10 => modified_immediate(first, second, rn),
        0b11 if op2 & 0b111_0001 == 0 => store_single(first, second, rn),
        0b11 if op2 & 0b110_0111 == 0b000_0111 => Flow::Stop.into(),
        0b11 if op2 & 0b110_0001 == 0b000_0001 => load_single(first, second, rn, pc),
        // Element and structure loads and stores write their base back when Rm is not the
        // program counter.
        0b11 if op2 & 0b111_0001 == 0b001_0000 => {
            let writeback = field(second.halfword, 0, 4) != 0xf;
            Effect::Writes(if writeback { register(rn) } else { 0 }).into()
        }
        // Data processing by register, and multiplies.
        0b11 if op2 & 0b111_0000 == 0b010_0000 || op2 & 0b111_1000 == 0b011_0000 => {
            Effect::Writes(register(second.rd)).into()
        }
        // Long multiplies write RdLo and RdHi; the divides only Rd.
        0b11 if op2 & 0b111_1000 == 0b011_1000 => Effect::Writes(match field(first, 4, 3) {
            0b001 | 0b011 => register(second.rd),
            _ => register(second.rt) | register(second.rd),
        })
        .into(),
        _ => coprocessor(first, second, rn).into(),
    }
}
