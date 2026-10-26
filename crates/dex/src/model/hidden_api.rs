// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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

impl HiddenApiFlags {
    pub const SDK: Self = Self(0);
    pub const BLOCKED: Self = Self(2);

    /// Preserves all bits, including flags introduced by newer ART versions.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}
