// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The 16-bit instructions.

use crate::arm::instruction::{
    Effect, Flow, Instruction, PC, Registers, SP, bit, field, register, register_at, sign_extend,
    word_aligned,
};

pub(in crate::arm) fn decode_narrow(halfword: u32, address: u64) -> Instruction {
    let pc = address + 4;
    let low = field(halfword, 0, 3) as u8;
    let high = field(halfword, 8, 3) as u8;
    let immediate = field(halfword, 0, 8);
    match halfword >> 11 {
        // Adds and subtracts of a register or a 3-bit immediate.
        0b00011 => {
            let base = field(halfword, 3, 3) as u8;
            let offset = field(halfword, 6, 3);
            match field(halfword, 9, 2) {
                0b10 => Effect::Add {
                    register: low,
                    base,
                    offset,
                },
                0b11 => Effect::Add {
                    register: low,
                    base,
                    offset: offset.wrapping_neg(),
                },
                _ => Effect::Writes(register(low)),
            }
            .into()
        }
        0b00100 => Effect::Constant {
            register: high,
            value: immediate,
        }
        .into(),
        0b00110 => Effect::Add {
            register: high,
            base: high,
            offset: immediate,
        }
        .into(),
        0b00111 => Effect::Add {
            register: high,
            base: high,
            offset: immediate.wrapping_neg(),
        }
        .into(),
        // TST, CMP and CMN only set flags.
        0b01000 if !bit(halfword, 10) => match field(halfword, 6, 4) {
            0b1000 | 0b1010 | 0b1011 => Effect::None.into(),
            _ => Effect::Writes(register(low)).into(),
        },
        0b01000 => special_data(halfword, pc),
        0b01001 => Effect::Literal {
            register: high,
            address: word_aligned(pc) + u64::from(immediate) * 4,
        }
        .into(),
        // Stores by register offset have opB 000 to 010; the rest of the group loads.
        0b01010 | 0b01011 if field(halfword, 9, 3) < 0b011 => Effect::None.into(),
        // Loads by immediate offset have L, bit 11, set.
        0b01100..=0b10001 if bit(halfword, 11) => Effect::Writes(register(low)).into(),
        // Shifts by an immediate, and loads by register offset.
        0b00000..=0b00010 | 0b01010 | 0b01011 => Effect::Writes(register(low)).into(),
        // Compares, stores by immediate offset and stores relative to the stack pointer.
        0b00101 | 0b01100..=0b10010 => Effect::None.into(),
        // Loads relative to the stack pointer, ADR, adds to the stack pointer and STM.
        0b10011..=0b10101 | 0b11000 => Effect::Writes(register(high)).into(),
        0b10110 | 0b10111 => miscellaneous(halfword, pc),
        // LDM writes its base back unless the list holds it.
        0b11001 => {
            let list = field(halfword, 0, 8) as Registers;
            let base = if list & register(high) == 0 {
                register(high)
            } else {
                0
            };
            Effect::Writes(list | base).into()
        }
        0b11010 | 0b11011 => match field(halfword, 8, 4) {
            0b1110 => Flow::Stop.into(),
            0b1111 => Effect::Writes(register(0)).into(),
            _ => Instruction::from(Flow::Branch(
                pc.wrapping_add_signed(sign_extend(immediate << 1, 9)),
            ))
            .conditional(),
        },
        0b11100 => {
            let offset = sign_extend(field(halfword, 0, 11) << 1, 12);
            Flow::Branch(pc.wrapping_add_signed(offset)).into()
        }
        _ => Flow::Stop.into(),
    }
}

/// Adds, compares and moves of any register, and BX and BLX.
fn special_data(halfword: u32, pc: u64) -> Instruction {
    let rd = (field(halfword, 7, 1) << 3 | field(halfword, 0, 3)) as u8;
    let rm = register_at(halfword, 3);
    match field(halfword, 8, 2) {
        0b00 | 0b10 if rd == PC => Flow::Stop.into(),
        0b00 if rm == PC => Effect::AddPc {
            register: rd,
            source: rd,
            pc,
        }
        .into(),
        0b01 => Effect::None.into(),
        0b00 | 0b10 => Effect::Writes(register(rd)).into(),
        _ if bit(halfword, 7) => Flow::IndirectCall.into(),
        _ => Flow::Stop.into(),
    }
}

fn miscellaneous(halfword: u32, pc: u64) -> Instruction {
    let low = field(halfword, 0, 3) as u8;
    match field(halfword, 8, 4) {
        // Adds to and subtracts from the stack pointer, and PUSH.
        0b0000 | 0b0100 | 0b0101 => Effect::Writes(register(SP)).into(),
        // CBZ and CBNZ branch forward by i:imm5:0.
        0b0001 | 0b0011 | 0b1001 | 0b1011 => {
            let offset = field(halfword, 9, 1) << 6 | field(halfword, 3, 5) << 1;
            Instruction::from(Flow::Branch(pc + u64::from(offset))).conditional()
        }
        // Extends and byte reversals.
        0b0010 | 0b1010 => Effect::Writes(register(low)).into(),
        0b1100 | 0b1101 if bit(halfword, 8) => Flow::Stop.into(),
        0b1100 | 0b1101 => Effect::Writes(field(halfword, 0, 8) as Registers | register(SP)).into(),
        0b1111 if field(halfword, 0, 4) != 0 => Flow::IfThen {
            count: 4 - field(halfword, 0, 4).trailing_zeros() as u8,
        }
        .into(),
        // CPS, SETEND and the hints.
        0b0110 | 0b1111 => Effect::None.into(),
        _ => Flow::Stop.into(),
    }
}
