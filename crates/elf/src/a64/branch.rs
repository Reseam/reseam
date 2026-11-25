// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AArch64 conditional branches, and the instructions that make one always or never jump.

use std::ops::Range;

use super::decode::sign_extend;
use crate::error::Result;
use crate::file::ElfImage;
use crate::{Branch, InstructionSet};

pub(crate) const NOP: u32 = 0xd503_201f;
const B: u32 = 0x1400_0000;

/// Where the conditional branch `instruction` at `pc` jumps: `b.cond`, `bc.cond`, `cbz`, `cbnz`,
/// `tbz` or `tbnz`. Each holds its offset in words from bit 5, 19 bits wide, or 14 in the test
/// branches.
fn conditional_target(instruction: u32, pc: u64) -> Option<u64> {
    let bits = match (instruction & 0xff00_0000, instruction & 0x7e00_0000) {
        (0x5400_0000, _) | (_, 0x3400_0000) => 19,
        (_, 0x3600_0000) => 14,
        _ => return None,
    };
    let words = sign_extend(u64::from(instruction >> 5) & ((1 << bits) - 1), bits);
    Some(pc.wrapping_add_signed(words << 2))
}

/// The conditional branches lying wholly in `range` of the code.
pub(crate) fn conditional_branches(image: &ElfImage, range: Range<u64>) -> Result<Vec<Branch>> {
    let start = range.start.next_multiple_of(4);
    let code = image.loaded(start, range.end.saturating_sub(start) as usize)?;
    Ok((code.as_chunks::<4>().0.iter())
        .zip((start..).step_by(4))
        .filter_map(|(instruction, address)| {
            Some(Branch {
                address,
                size: 4,
                target: conditional_target(u32::from_le_bytes(*instruction), address)?,
                instruction_set: InstructionSet::A64,
            })
        })
        .collect())
}

/// `b` from `from` to `to`, whose range of 128 MiB covers every conditional branch's.
pub(crate) fn jump(from: u64, to: u64) -> u32 {
    B | ((to.wrapping_sub(from) >> 2) as u32 & 0x03ff_ffff)
}
