// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data processing, multiplies, media and miscellaneous instructions.

use super::fields::{expand_immediate, rd, rm, rn, wide_immediate};
use super::memory::extra_load_store;
use crate::arm::instruction::{ALL, Effect, Flow, Instruction, PC, bit, field, register};

const ADD: u32 = 0b0100;
const SUB: u32 = 0b0010;
const MOV: u32 = 0b1101;
const MVN: u32 = 0b1111;

/// The second operand of a data-processing instruction.
#[derive(Clone, Copy)]
enum Operand {
    Immediate(u32),
    /// A register, with a shift applied or not.
    Register {
        index: u8,
        shifted: bool,
    },
}

pub(super) fn data_and_miscellaneous(word: u32, pc: u64) -> Instruction {
    let op = field(word, 20, 5);
    let op2 = field(word, 4, 4);
    if bit(word, 25) {
        return match op {
            0b10000 => Effect::Constant {
                register: rd(word),
                value: wide_immediate(word),
            }
            .into(),
            0b10100 => Effect::High {
                register: rd(word),
                value: wide_immediate(word),
            }
            .into(),
            // MSR immediate and the hints.
            _ if op & 0b11011 == 0b10010 => Effect::None.into(),
            _ => data_processing(word, pc, Operand::Immediate(expand_immediate(word))),
        };
    }
    match op2 {
        // Multiplies write Rd in bits 19..16; the long forms and UMAAL also RdLo in 15..12. The
        // synchronization group writes Rt or the status register in 15..12, LDREXD also Rt + 1.
        0b1001 => Effect::Writes(match op {
            _ if op & 0b10000 == 0 && (op & 0b01000 != 0 || op == 0b00100) => {
                register(rn(word)) | register(rd(word))
            }
            _ if op & 0b10000 == 0 => register(rn(word)),
            0b11011 => register(rd(word)) | register((rd(word) + 1) & 0xf),
            _ => register(rd(word)),
        })
        .into(),
        _ if op2 & 0b1001 == 0b1001 => extra_load_store(word),
        _ if op & 0b11001 == 0b10000 => miscellaneous(word, op2),
        _ => data_processing(
            word,
            pc,
            Operand::Register {
                index: rm(word),
                shifted: field(word, 4, 8) != 0,
            },
        ),
    }
}

/// TST, TEQ, CMP and CMN only set flags; a write to the program counter jumps.
fn data_processing(word: u32, pc: u64, operand: Operand) -> Instruction {
    let opcode = field(word, 21, 4);
    let (rd, rn) = (rd(word), rn(word));
    if opcode & 0b1100 == 0b1000 {
        return Effect::None.into();
    }
    if rd == PC {
        return Flow::Stop.into();
    }
    let effect = match (opcode, operand) {
        (ADD | SUB, Operand::Immediate(_)) if rn == PC => Effect::Writes(register(rd)),
        (ADD, Operand::Immediate(value)) => Effect::Add {
            register: rd,
            base: rn,
            offset: value,
        },
        (SUB, Operand::Immediate(value)) => Effect::Add {
            register: rd,
            base: rn,
            offset: value.wrapping_neg(),
        },
        (MOV, Operand::Immediate(value)) => Effect::Constant {
            register: rd,
            value,
        },
        (MVN, Operand::Immediate(value)) => Effect::Constant {
            register: rd,
            value: !value,
        },
        (
            ADD,
            Operand::Register {
                index,
                shifted: false,
            },
        ) if rn == PC || index == PC => Effect::AddPc {
            register: rd,
            source: if rn == PC { index } else { rn },
            pc,
        },
        _ => Effect::Writes(register(rd)),
    };
    effect.into()
}

fn miscellaneous(word: u32, op2: u32) -> Instruction {
    let op = field(word, 21, 2);
    match (op2, op) {
        // Halfword multiplies write bits 19..16; SMLALxy also RdLo in 15..12.
        _ if op2 & 0b1001 == 0b1000 => {
            let low = if op == 0b10 { register(rd(word)) } else { 0 };
            Effect::Writes(register(rn(word)) | low).into()
        }
        (0b0000, 0b00 | 0b10) | (0b0001, 0b11) | (0b0101, _) => {
            Effect::Writes(register(rd(word))).into()
        }
        (0b0000, _) => Effect::None.into(),
        (0b0001 | 0b0010 | 0b0111, 0b01) | (0b0110, 0b11) => Flow::Stop.into(),
        (0b0011, 0b01) => Flow::IndirectCall.into(),
        _ => Effect::Writes(ALL).into(),
    }
}

pub(super) fn media(word: u32) -> Instruction {
    let op1 = field(word, 20, 5);
    if op1 == 0b11111 && field(word, 5, 3) == 0b111 {
        return Flow::Stop.into();
    }
    Effect::Writes(match op1 >> 3 {
        // Signed multiplies and divides write bits 19..16; SMLALD and SMLSLD also 15..12.
        0b10 if op1 == 0b10100 => register(rn(word)) | register(rd(word)),
        0b10 => register(rn(word)),
        // USAD8 and USADA8 write bits 19..16, the bitfield instructions 15..12.
        0b11 if op1 == 0b11000 => register(rn(word)),
        _ => register(rd(word)),
    })
    .into()
}
