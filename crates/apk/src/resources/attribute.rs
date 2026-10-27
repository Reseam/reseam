// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EntryValue, MapEntry, ResourceTable};
use crate::error::Result;
use crate::res_value::ResValue;

/// The bag item of an `attr` entry that holds its formats.
const ATTR_TYPE: u32 = 0x0100_0000;

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

/// The value an `attr` gives one of its enum or flag names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrSymbol {
    pub value: u32,
    pub flags: bool,
}

/// An `attr` definition: its formats and every bag item, the formats included.
struct AttrBag {
    formats: AttrFormats,
    items: Vec<MapEntry>,
}

impl ResourceTable {
    /// Reads the permitted formats of a local attribute definition lazily.
    /// An absent definition or format item has no declared constraint; corrupt
    /// entries are errors. Attribute definitions are configuration independent.
    pub fn attr_formats(&self, attr_id: u32) -> Result<Option<AttrFormats>> {
        Ok(self.attr_bag(attr_id)?.map(|bag| bag.formats))
    }

    /// The value the app's `attr` `attr_id` gives the enum or flag name
    /// `symbol`; `None` when its format takes neither or it has no such name.
    pub fn attr_symbol(&self, attr_id: u32, symbol: &str) -> Result<Option<AttrSymbol>> {
        let Some(bag) = self.attr_bag(attr_id)? else {
            return Ok(None);
        };
        let flags = bag.formats.contains(AttrFormats::FLAGS);
        if !flags && !bag.formats.contains(AttrFormats::ENUM) {
            return Ok(None);
        }
        for item in bag.items {
            if self.entry_name(item.name)?.as_deref() == Some(symbol) {
                return Ok(Some(AttrSymbol {
                    value: item.value.data,
                    flags,
                }));
            }
        }
        Ok(None)
    }

    /// The `attr` id a bag item name refers to: `android:name` reads the
    /// framework table, an unprefixed name the app's own `attr` entries, which
    /// is what a `<style>` item name means in resource XML.
    pub fn attr_id(&self, name: &str) -> Result<Option<u32>> {
        match name.strip_prefix("android:") {
            Some(local) => Ok(crate::axml::android_attr_res_id(local)),
            None => self.find_resource_id("attr", name),
        }
    }

    fn attr_bag(&self, attr_id: u32) -> Result<Option<AttrBag>> {
        let Some((_, entry)) = self.default_entry(attr_id)? else {
            return Ok(None);
        };
        let EntryValue::Complex { entries, .. } = entry.value else {
            return Ok(None);
        };
        let formats = entries
            .iter()
            .find(|item| item.name == ATTR_TYPE)
            .map(|item| AttrFormats::from_bits_retain(item.value.data));
        Ok(formats.map(|formats| AttrBag {
            formats,
            items: entries,
        }))
    }
}
