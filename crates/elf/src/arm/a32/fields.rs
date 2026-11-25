// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The register and immediate fields A32 encodings share.

use crate::arm::instruction::{field, register_at, sign_extend};

/// Rd, or Rt, in bits 15..12.
pub(super) fn rd(word: u32) -> u8 {
    register_at(word, 12)
}

/// Rn in bits 19..16, which some multiplies use as Rd.
pub(super) fn rn(word: u32) -> u8 {
    register_at(word, 16)
}

/// Rm in bits 3..0.
pub(super) fn rm(word: u32) -> u8 {
    register_at(word, 0)
}

/// The 16-bit immediate of `movw` and `movt`, split as imm4 in bits 19..16 and imm12 in 11..0.
pub(super) fn wide_immediate(word: u32) -> u32 {
    field(word, 16, 4) << 12 | field(word, 0, 12)
}

/// `opcode` with `value` as its 16-bit immediate, split as [`wide_immediate`] reads it.
pub(super) fn with_wide_immediate(opcode: u32, value: u32) -> u32 {
    opcode | field(value, 12, 4) << 16 | field(value, 0, 12)
}

/// imm24, the offset of `b`, `bl` and `blx` in words, in bytes.
pub(super) fn branch_offset(word: u32) -> i64 {
    sign_extend(word << 2, 26)
}

/// `opcode` with `offset`, in bytes, split as [`branch_offset`] reads it.
pub(super) fn with_branch_offset(opcode: u32, offset: i64) -> u32 {
    opcode | field((offset >> 2) as u32, 0, 24)
}

/// `ARMExpandImm`: an 8-bit value rotated right by twice the 4-bit rotation above it.
pub(super) fn expand_immediate(word: u32) -> u32 {
    field(word, 0, 8).rotate_right(2 * field(word, 8, 4))
}
