// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 32-bit ARM libraries, which mix A32 and Thumb functions. Their unwind table cannot bound
//! functions: it has no sizes, no instruction sets, and linkers merge the entries of neighbouring
//! functions that unwind alike. Functions are found by walking the code instead, and code computes
//! a data address by adding the program counter to a distance it loads.

mod a32;
mod blocks;
mod code;
mod decode;
mod discovery;
mod exidx;
mod instruction;
mod map;
mod mode;
mod relocations;
mod seeds;
mod t32;
mod tracking;

pub(crate) use code::CodeMap;
pub(crate) use discovery::walk;
pub(crate) use mode::{Mode, code_pointer};

use crate::Branch;

/// The code that sets `r0` to `value` and returns, in `mode`.
pub(crate) fn return_code(mode: Mode, value: i32) -> Vec<u8> {
    match mode {
        Mode::Arm => a32::return_code(value),
        Mode::Thumb => t32::return_code(value),
    }
}

/// The code that replaces `branch`, in `mode`, with one that always jumps.
pub(crate) fn jump(mode: Mode, branch: Branch) -> Vec<u8> {
    match mode {
        Mode::Arm => a32::jump(branch.address, branch.target),
        Mode::Thumb => t32::jump(branch.address, branch.target, branch.size),
    }
}

/// The code that replaces `branch`, in `mode`, with one that never jumps.
pub(crate) fn nop(mode: Mode, branch: Branch) -> Vec<u8> {
    match mode {
        Mode::Arm => a32::nop(),
        Mode::Thumb => t32::nop(branch.size),
    }
}
