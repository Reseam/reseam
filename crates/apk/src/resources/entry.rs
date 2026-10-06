// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod reader;
mod writer;
use crate::value::ResValue;
pub(super) use reader::{entry_head, entry_len, parse_entry};
pub(super) use writer::serialize_entry;

const FLAG_COMPLEX: u16 = 0x0001;
pub(super) const FLAG_COMPACT: u16 = 0x0008;
const SIMPLE_LEN: usize = 8;
const COMPLEX_LEN: usize = 16;
const MAP_ENTRY_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct ResEntry {
    pub flags: u16,
    pub key: u32,
    pub value: EntryValue,
}

#[derive(Debug, Clone)]
pub enum EntryValue {
    Simple(ResValue),
    Complex { parent: u32, entries: Vec<MapEntry> },
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub name: u32,
    pub value: ResValue,
}
