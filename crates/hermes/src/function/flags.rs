// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};

/// Which invocations a function refuses (`ProhibitInvoke`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prohibit {
    Call,
    Construct,
    Neither,
}

/// `FuncKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionKind {
    Normal,
    Generator,
    Async,
}

/// `FunctionHeaderFlag`, without the overflowed bit, which follows the header's encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionFlags {
    pub prohibit: Prohibit,
    pub strict: bool,
    pub exception_handler: bool,
    pub debug_info: bool,
    pub kind: FunctionKind,
}

impl FunctionFlags {
    const STRICT: u8 = 1 << 2;
    const EXCEPTION_HANDLER: u8 = 1 << 3;
    const DEBUG_INFO: u8 = 1 << 4;
    pub(super) const OVERFLOWED: u8 = 1 << 5;

    pub(super) fn decode(byte: u8, offset: usize) -> Result<Self> {
        Ok(Self {
            prohibit: match byte & 3 {
                0 => Prohibit::Call,
                1 => Prohibit::Construct,
                2 => Prohibit::Neither,
                _ => return Err(invalid(offset, "invalid prohibited invocation")),
            },
            strict: byte & Self::STRICT != 0,
            exception_handler: byte & Self::EXCEPTION_HANDLER != 0,
            debug_info: byte & Self::DEBUG_INFO != 0,
            kind: match byte >> 6 {
                0 => FunctionKind::Normal,
                1 => FunctionKind::Generator,
                2 => FunctionKind::Async,
                _ => return Err(invalid(offset, "invalid function kind")),
            },
        })
    }

    pub(super) fn encode(self, overflowed: bool) -> u8 {
        let prohibit = match self.prohibit {
            Prohibit::Call => 0,
            Prohibit::Construct => 1,
            Prohibit::Neither => 2,
        };
        let kind = match self.kind {
            FunctionKind::Normal => 0,
            FunctionKind::Generator => 1,
            FunctionKind::Async => 2,
        };
        let bit = |set: bool, bit: u8| if set { bit } else { 0 };
        prohibit
            | bit(self.strict, Self::STRICT)
            | bit(self.exception_handler, Self::EXCEPTION_HANDLER)
            | bit(self.debug_info, Self::DEBUG_INFO)
            | bit(overflowed, Self::OVERFLOWED)
            | kind << 6
    }
}
