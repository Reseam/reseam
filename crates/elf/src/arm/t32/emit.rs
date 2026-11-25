// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::fields::{with_branch_offset, with_wide_immediate};
use crate::arm::instruction::field;

const MOVS_R0: u16 = 0x2000;
const MVN_R0: u16 = 0xf06f;
/// `movw` and `movt` take Rd in the second halfword, which a zero leaves as r0.
const MOVW: u16 = 0xf240;
const MOVT: u16 = 0xf2c0;
const BX_LR: u16 = 0x4770;
const NOP: u16 = 0xbf00;
const NOP_WIDE: [u16; 2] = [0xf3af, 0x8000];
const B: u16 = 0xe000;
const B_WIDE: [u16; 2] = [0xf000, 0x9000];

/// The code that sets `r0` to `value` and returns: `movs` or `mvn` of an 8-bit immediate, or
/// `movw` with a `movt` when the top half is set, then `bx lr`.
pub(in crate::arm) fn return_code(value: i32) -> Vec<u8> {
    let value = value as u32;
    let set = match value {
        0..=0xff => vec![MOVS_R0 | value as u16],
        0x100..=0xffff => with_wide_immediate(MOVW, value).to_vec(),
        _ if !value <= 0xff => vec![MVN_R0, !value as u16],
        _ => [
            with_wide_immediate(MOVW, value & 0xffff),
            with_wide_immediate(MOVT, value >> 16),
        ]
        .concat(),
    };
    bytes(set.into_iter().chain([BX_LR]))
}

/// A `nop` of `size` bytes.
pub(in crate::arm) fn nop(size: u64) -> Vec<u8> {
    if size == 2 {
        bytes([NOP])
    } else {
        bytes(NOP_WIDE)
    }
}

/// A `b` from `from` to `to` in `size` bytes. The 16-bit form reaches 2 KiB and the 32-bit one
/// 16 MiB, which cover the conditional branches of the same size: 16-bit `b<c>`, `cbz` and `cbnz`
/// reach 256 bytes, and 32-bit `b<c>` 1 MiB. Reading the program counter gives `from` plus 4.
pub(in crate::arm) fn jump(from: u64, to: u64, size: u64) -> Vec<u8> {
    let offset = to.wrapping_sub(from + 4) as i64;
    if size == 2 {
        bytes([B | field(offset as u32, 1, 11) as u16])
    } else {
        bytes(with_branch_offset(B_WIDE, offset))
    }
}

/// Thumb code as bytes: a 32-bit instruction is two halfwords, the first one first.
fn bytes(halfwords: impl IntoIterator<Item = u16>) -> Vec<u8> {
    halfwords.into_iter().flat_map(u16::to_le_bytes).collect()
}
