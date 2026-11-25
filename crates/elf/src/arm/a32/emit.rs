// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::fields::{with_branch_offset, with_wide_immediate};

const MOVW_R0: u32 = 0xe300_0000;
const MOVT_R0: u32 = 0xe340_0000;
const MVN_R0: u32 = 0xe3e0_0000;
const BX_LR: u32 = 0xe12f_ff1e;
const NOP: u32 = 0xe320_f000;
/// `b` with the condition set to always.
const B: u32 = 0xea00_0000;

/// The code that sets `r0` to `value` and returns: `mvn` of an 8-bit immediate, or `movw` with
/// a `movt` when the top half is set, then `bx lr`.
pub(in crate::arm) fn return_code(value: i32) -> Vec<u8> {
    let value = value as u32;
    let set = match value {
        0..=0xffff => vec![with_wide_immediate(MOVW_R0, value)],
        _ if !value <= 0xff => vec![MVN_R0 | !value],
        _ => vec![
            with_wide_immediate(MOVW_R0, value & 0xffff),
            with_wide_immediate(MOVT_R0, value >> 16),
        ],
    };
    bytes(set.into_iter().chain([BX_LR]))
}

pub(in crate::arm) fn nop() -> Vec<u8> {
    bytes([NOP])
}

/// `b` from `from` to `to`, which reaches as far as a conditional `b`. Reading the program counter
/// gives `from` plus 8.
pub(in crate::arm) fn jump(from: u64, to: u64) -> Vec<u8> {
    bytes([with_branch_offset(B, to.wrapping_sub(from + 8) as i64)])
}

fn bytes(words: impl IntoIterator<Item = u32>) -> Vec<u8> {
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}
