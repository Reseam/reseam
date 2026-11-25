// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fields of 32-bit Thumb encodings, which span two halfwords.

use crate::arm::instruction::{field, register_at, sign_extend};

/// The fields the second halfword of a 32-bit instruction holds.
#[derive(Clone, Copy)]
pub(super) struct Second {
    pub(super) halfword: u32,
    /// Rt in bits 15..12.
    pub(super) rt: u8,
    /// Rd, or Rt2, in bits 11..8.
    pub(super) rd: u8,
}

impl Second {
    pub(super) fn new(halfword: u32) -> Self {
        Self {
            halfword,
            rt: register_at(halfword, 12),
            rd: register_at(halfword, 8),
        }
    }
}

/// i:imm3:imm8, the 12-bit immediate of data processing, split over bit 10 of the first
/// halfword and the second.
pub(super) fn immediate12(first: u32, second: u32) -> u32 {
    field(first, 10, 1) << 11 | field(second, 12, 3) << 8 | field(second, 0, 8)
}

/// imm4:i:imm3:imm8, the 16-bit immediate of `movw` and `movt`.
pub(super) fn wide_immediate(first: u32, second: u32) -> u32 {
    field(first, 0, 4) << 12 | immediate12(first, second)
}

/// `opcode`'s two halfwords with `value` as their 16-bit immediate, split as [`wide_immediate`]
/// reads it.
pub(super) fn with_wide_immediate(opcode: u16, value: u32) -> [u16; 2] {
    [
        opcode | (field(value, 11, 1) << 10 | field(value, 12, 4)) as u16,
        (field(value, 8, 3) << 12 | field(value, 0, 8)) as u16,
    ]
}

/// S:I1:I2:imm10:imm11:0, the offset of `b.w`, `bl` and `blx`, with I1 = NOT(J1 XOR S) and
/// I2 = NOT(J2 XOR S).
pub(super) fn branch_offset(first: u32, second: u32) -> i64 {
    let sign = field(first, 10, 1);
    let i1 = 1 ^ field(second, 13, 1) ^ sign;
    let i2 = 1 ^ field(second, 11, 1) ^ sign;
    let value =
        sign << 24 | i1 << 23 | i2 << 22 | field(first, 0, 10) << 12 | field(second, 0, 11) << 1;
    sign_extend(value, 25)
}

/// `opcode`'s two halfwords with `offset` split as [`branch_offset`] reads it.
pub(super) fn with_branch_offset(opcode: [u16; 2], offset: i64) -> [u16; 2] {
    let offset = offset as u32;
    let sign = field(offset, 24, 1);
    let j1 = 1 ^ field(offset, 23, 1) ^ sign;
    let j2 = 1 ^ field(offset, 22, 1) ^ sign;
    [
        opcode[0] | (sign << 10 | field(offset, 12, 10)) as u16,
        opcode[1] | (j1 << 13 | j2 << 11 | field(offset, 1, 11)) as u16,
    ]
}
