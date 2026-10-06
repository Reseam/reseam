// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{ATTR_TYPE, EntryValue, ResourceTable};
use crate::error::Result;
use crate::value::ResValue;

bitflags::bitflags! {
    /// Formats declared by an Android attribute's `ATTR_TYPE` bag item.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AttrFormats: u32 {
        const REFERENCE = 1;
        const STRING = 1 << 1;
        const INTEGER = 1 << 2;
        const BOOLEAN = 1 << 3;
        const COLOR = 1 << 4;
        const FLOAT = 1 << 5;
        const DIMENSION = 1 << 6;
        const FRACTION = 1 << 7;
        const ENUM = 1 << 16;
        const FLAGS = 1 << 17;
    }
}

impl AttrFormats {
    pub(crate) fn accepts(self, value: ResValue) -> bool {
        let format = match value.kind {
            0 | ResValue::REFERENCE | ResValue::ATTRIBUTE => return true,
            ResValue::STRING => Self::STRING,
            ResValue::INT_DEC | ResValue::INT_HEX => Self::INTEGER | Self::ENUM | Self::FLAGS,
            ResValue::INT_BOOLEAN => Self::BOOLEAN,
            ResValue::FLOAT => Self::FLOAT,
            ResValue::DIMENSION => Self::DIMENSION,
            ResValue::FRACTION => Self::FRACTION,
            ResValue::INT_COLOR_ARGB8..=ResValue::INT_COLOR_RGB4 => Self::COLOR,
            _ => return false,
        };
        self.intersects(format)
    }
}

impl ResourceTable {
    /// Reads the permitted formats of a local attribute definition lazily.
    /// An absent definition or format item has no declared constraint; corrupt
    /// entries are errors. Attribute definitions are configuration independent.
    pub fn attr_formats(&self, attr_id: u32) -> Result<Option<AttrFormats>> {
        let Some((_, entry)) = self.default_entry(attr_id)? else {
            return Ok(None);
        };
        let EntryValue::Complex { entries, .. } = entry.value else {
            return Ok(None);
        };
        Ok(entries
            .iter()
            .find(|item| item.name == ATTR_TYPE)
            .map(|item| AttrFormats::from_bits_retain(item.value.data)))
    }
}
