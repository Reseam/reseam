// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::res_value::ResValue;

/// `0xPPTTEEEE`: package, type and entry index.
pub(super) fn res_id(package_id: u32, type_id: u8, entry_index: usize) -> u32 {
    (package_id << 24) | (u32::from(type_id) << 16) | (entry_index as u32)
}

#[derive(Clone, Copy)]
pub(super) struct ResourceAddress {
    pub(super) package_id: u32,
    pub(super) type_id: u8,
    pub(super) entry_index: usize,
}

impl ResourceAddress {
    pub(super) fn decode(id: u32) -> Self {
        Self {
            package_id: id >> 24,
            type_id: (id >> 16) as u8,
            entry_index: (id & 0xffff) as usize,
        }
    }
}

/// An entry found by name, with its simple value when it has one.
#[derive(Debug, Clone, Copy)]
pub(super) struct EntryLocation {
    pub(super) package_id: u32,
    pub(super) type_id: u8,
    pub(super) entry_index: usize,
    pub(super) value: Option<ResValue>,
}

impl EntryLocation {
    pub(super) fn res_id(self) -> u32 {
        res_id(self.package_id, self.type_id, self.entry_index)
    }
}
