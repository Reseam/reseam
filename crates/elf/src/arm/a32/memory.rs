// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Loads and stores, and transfers to and from coprocessor and floating-point registers.

use super::fields::{rd, rm, rn};
use crate::arm::instruction::{Effect, Flow, Instruction, PC, Registers, bit, field, register};

/// Halfword, signed byte and doubleword transfers. LDRD and STRD clear the load bit and tell
/// themselves apart by bits 6..5.
pub(super) fn extra_load_store(word: u32) -> Instruction {
    let (rt, rn) = (rd(word), rn(word));
    let op2 = field(word, 5, 2);
    let load = bit(word, 20) || op2 == 0b10;
    let pair = !bit(word, 20) && op2 & 0b10 != 0;
    let writeback = (!bit(word, 24) || bit(word, 21)) && rn != PC;
    let base = if writeback { register(rn) } else { 0 };
    let loaded = match (load, pair) {
        (true, true) => register(rt) | register((rt + 1) & 0xf),
        (true, false) => register(rt),
        (false, _) => 0,
    };
    Effect::Writes(base | loaded).into()
}

/// LDR, LDRB, STR and STRB. A word load into the program counter jumps.
pub(super) fn load_store(word: u32, pc: u64) -> Instruction {
    let (rt, rn) = (rd(word), rn(word));
    let load = bit(word, 20);
    let byte = bit(word, 22);
    let indexed = bit(word, 24);
    let writeback = !indexed || bit(word, 21);
    if !load {
        return Effect::Writes(if writeback { register(rn) } else { 0 }).into();
    }
    if rt == PC && !byte {
        return Flow::Stop.into();
    }
    let from_pc = rn == PC && !byte && indexed && !writeback;
    match bit(word, 25) {
        false if from_pc => {
            let offset = u64::from(field(word, 0, 12));
            Effect::Literal {
                register: rt,
                address: if bit(word, 23) {
                    pc.wrapping_add(offset)
                } else {
                    pc.wrapping_sub(offset)
                },
            }
            .into()
        }
        true if from_pc && bit(word, 23) && field(word, 4, 8) == 0 => Effect::LoadPcIndexed {
            register: rt,
            index: rm(word),
            pc,
        }
        .into(),
        _ if writeback && rn != PC => Instruction::load(register(rt) | register(rn), rn, true),
        _ => Effect::Writes(register(rt)).into(),
    }
}

/// LDM and STM, including PUSH and POP. Loading the program counter returns or jumps.
pub(super) fn block_transfer(word: u32) -> Instruction {
    let list = field(word, 0, 16) as Registers;
    let writeback = bit(word, 21);
    let base = if writeback { register(rn(word)) } else { 0 };
    match bit(word, 20) {
        false => Effect::Writes(base).into(),
        true if list & register(PC) != 0 => Flow::Stop.into(),
        true => Instruction::load(list | base, rn(word), writeback),
    }
}

/// Supervisor calls and the coprocessor, floating-point and SIMD register transfers.
pub(super) fn coprocessor(word: u32) -> Effect {
    let (rt, rn) = (rd(word), rn(word));
    let load = bit(word, 20);
    let two_registers = field(word, 21, 7) == 0b110_0010;
    match field(word, 24, 4) {
        0b1111 => Effect::Writes(register(0)),
        0b1110 if bit(word, 4) && load && rt != PC => Effect::Writes(register(rt)),
        0b1110 => Effect::None,
        _ if two_registers && load => Effect::Writes(register(rt) | register(rn)),
        _ if two_registers => Effect::None,
        _ if bit(word, 21) => Effect::Writes(register(rn)),
        _ => Effect::None,
    }
}
