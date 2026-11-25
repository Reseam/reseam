// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 32-bit loads and stores, and transfers to and from coprocessor and floating-point registers.

use super::fields::Second;
use crate::arm::instruction::{
    ALL, Effect, Flow, Instruction, PC, Registers, SP, bit, field, register, register_at,
    word_aligned,
};

/// LDM and STM, including PUSH and POP. Loading the program counter returns or jumps.
pub(super) fn load_store_multiple(first: u32, second: Second, rn: u8) -> Instruction {
    let load = bit(first, 4);
    let writeback = bit(first, 5);
    let base = if writeback { register(rn) } else { 0 };
    match field(first, 7, 2) {
        0b01 | 0b10 if load && bit(second.halfword, 15) => Flow::Stop.into(),
        0b01 | 0b10 if load => {
            Instruction::load(second.halfword as Registers | base, rn, writeback)
        }
        0b01 | 0b10 => Effect::Writes(base).into(),
        // RFE returns from an exception; SRS stores to the stack.
        _ if load => Flow::Stop.into(),
        _ => Effect::Writes(register(SP)).into(),
    }
}

/// Doubleword and exclusive transfers, and the table branches.
pub(super) fn dual_exclusive_table(first: u32, second: Second, rn: u8, pc: u64) -> Instruction {
    let base = if bit(first, 5) { register(rn) } else { 0 };
    let (rt, rt2) = (register(second.rt), register(second.rd));
    match (field(first, 7, 2), field(first, 4, 2)) {
        // STREX writes its status register; LDREX its target.
        (0b00, 0b00) => Effect::Writes(rt2).into(),
        (0b00, 0b01) => Effect::Writes(rt).into(),
        // STRD and LDRD with an immediate.
        (0b00 | 0b01, 0b10) | (0b10 | 0b11, 0b00 | 0b10) => Effect::Writes(base).into(),
        (0b00 | 0b01, 0b11) | (0b10 | 0b11, 0b01 | 0b11) => Effect::Writes(rt | rt2 | base).into(),
        // STREXB, STREXH and STREXD write their status register.
        (0b01, 0b00) => Effect::Writes(register(register_at(second.halfword, 0))).into(),
        _ => match field(second.halfword, 4, 4) {
            0b0000 | 0b0001 if rn == PC => Flow::Table {
                base: pc,
                halfwords: bit(second.halfword, 4),
            }
            .into(),
            0b0000 | 0b0001 => Flow::Stop.into(),
            // LDREXD loads two registers, LDREXB and LDREXH one.
            0b0111 => Effect::Writes(rt | rt2).into(),
            _ => Effect::Writes(rt).into(),
        },
    }
}

/// Whether an 8-bit immediate form, which hw1 bit 7 clear and hw2 bit 11 set mark, writes its
/// base back: its W bit, hw2 bit 8.
fn writes_back(first: u32, second: Second) -> bool {
    !bit(first, 7) && bit(second.halfword, 11) && bit(second.halfword, 8)
}

/// STR, STRB and STRH.
pub(super) fn store_single(first: u32, second: Second, rn: u8) -> Instruction {
    Effect::Writes(if writes_back(first, second) {
        register(rn)
    } else {
        0
    })
    .into()
}

/// LDR, LDRB, LDRH and their signed forms, and the preload hints, which name the program
/// counter as Rt. A word load into the program counter jumps.
pub(super) fn load_single(first: u32, second: Second, rn: u8, pc: u64) -> Instruction {
    let word = field(first, 5, 2) == 0b10;
    let rt = second.rt;
    match rt {
        PC if word => Flow::Stop.into(),
        PC => Effect::None.into(),
        _ if rn == PC && word && !bit(first, 8) => {
            let offset = u64::from(field(second.halfword, 0, 12));
            let base = word_aligned(pc);
            Effect::Literal {
                register: rt,
                address: if bit(first, 7) {
                    base + offset
                } else {
                    base.wrapping_sub(offset)
                },
            }
            .into()
        }
        _ => {
            let writeback = rn != PC && writes_back(first, second);
            let base = if writeback { register(rn) } else { 0 };
            Instruction::load(register(rt) | base, rn, writeback)
        }
    }
}

/// Coprocessor, floating-point and SIMD instructions. Only transfers to general registers and
/// base writeback touch them.
pub(super) fn coprocessor(first: u32, second: Second, rn: u8) -> Effect {
    let op1 = field(first, 4, 6);
    let load = bit(first, 4);
    match op1 {
        // Advanced SIMD data processing.
        _ if first & 0xef00 == 0xef00 => Effect::None,
        // MRRC and VMOV to two general registers.
        0b00_0100 | 0b00_0101 if load => Effect::Writes(register(second.rt) | register(rn)),
        0b00_0100 | 0b00_0101 => Effect::None,
        // LDC, STC, VLDR, VSTR, VLDM and VSTM write their base back when W is set.
        _ if op1 & 0b10_0000 == 0 && bit(first, 5) => Effect::Writes(register(rn)),
        _ if op1 & 0b10_0000 == 0 => Effect::None,
        // MRC and VMOV to a general register.
        _ if op1 & 0b11_0000 == 0b10_0000 && bit(second.halfword, 4) && load && second.rt != PC => {
            Effect::Writes(register(second.rt))
        }
        _ if op1 & 0b11_0000 == 0b10_0000 => Effect::None,
        _ => Effect::Writes(ALL),
    }
}
