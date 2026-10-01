// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{FieldIdx, MethodIdx};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ClassHiddenApiFlags {
    pub field_flags: BTreeMap<FieldIdx, HiddenApiFlags>,
    pub method_flags: BTreeMap<MethodIdx, HiddenApiFlags>,
}

/// Hidden-API access restriction and domain bits, retained verbatim for ART.
/// The low nibble is the restriction; domain and future flags occupy higher bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HiddenApiFlags(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiddenApiRestriction {
    Sdk,
    Unsupported,
    Blocked,
    MaxO,
    MaxP,
    MaxQ,
    MaxR,
    MaxS,
}

impl HiddenApiFlags {
    pub const SDK: Self = Self(0);
    pub const UNSUPPORTED: Self = Self(1);
    pub const BLOCKED: Self = Self(2);

    /// Preserves all bits, including flags introduced by newer ART versions.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    /// ART treats an unknown restriction value as unsupported.
    pub const fn restriction(self) -> HiddenApiRestriction {
        match self.0 & 0xf {
            0 => HiddenApiRestriction::Sdk,
            2 => HiddenApiRestriction::Blocked,
            3 => HiddenApiRestriction::MaxO,
            4 => HiddenApiRestriction::MaxP,
            5 => HiddenApiRestriction::MaxQ,
            6 => HiddenApiRestriction::MaxR,
            7 => HiddenApiRestriction::MaxS,
            _ => HiddenApiRestriction::Unsupported,
        }
    }

    pub const fn is_core_platform_api(self) -> bool {
        self.0 & 0x10 != 0
    }
    pub const fn is_test_api(self) -> bool {
        self.0 & 0x20 != 0
    }
}
