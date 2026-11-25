// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::InstructionSet;

/// The instruction set of 32-bit ARM code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Arm,
    Thumb,
}

impl Mode {
    pub(super) fn alignment(self) -> u64 {
        match self {
            Self::Arm => 4,
            Self::Thumb => 2,
        }
    }
}

impl From<Mode> for InstructionSet {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Arm => Self::A32,
            Mode::Thumb => Self::T32,
        }
    }
}

/// The code a pointer addresses. Its low bit marks Thumb code and is not part of the address.
pub(crate) fn code_pointer(value: u64) -> (u64, Mode) {
    if value & 1 == 1 {
        (value & !1, Mode::Thumb)
    } else {
        (value, Mode::Arm)
    }
}
