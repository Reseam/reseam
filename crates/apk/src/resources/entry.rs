// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::Result;
use crate::value::ResValue;

const FLAG_COMPLEX: u16 = 0x0001;
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

pub(super) fn entry_len(chunk: &[u8], pos: usize) -> Result<usize> {
    require_len(chunk, pos, SIMPLE_LEN, "res entry")?;
    let entry_size = read_u16_le(chunk, pos, "res entry")? as usize;
    let flags = read_u16_le(chunk, pos + 2, "res entry")?;
    let total = if flags & FLAG_COMPLEX != 0 {
        require_len(chunk, pos, COMPLEX_LEN, "complex res entry")?;
        let count = read_u32_le(chunk, pos + 12, "res entry count")? as usize;
        entry_size.max(COMPLEX_LEN) + count * MAP_ENTRY_LEN
    } else {
        entry_size.max(SIMPLE_LEN) + SIMPLE_LEN
    };
    require_len(chunk, pos, total, "res entry")?;
    Ok(total)
}

/// The key and, for a simple entry, the value, without decoding a map.
pub(super) fn entry_head(bytes: &[u8]) -> Option<(u32, Option<ResValue>)> {
    let flags = read_u16_le(bytes, 2, "res entry").ok()?;
    let key = read_u32_le(bytes, 4, "res entry").ok()?;
    let value = (flags & FLAG_COMPLEX == 0)
        .then(|| ResValue::new(bytes[11], read_u32_le(bytes, 12, "res value").unwrap_or(0)));
    Some((key, value))
}

pub(super) fn parse_entry(bytes: &[u8]) -> Result<ResEntry> {
    let flags = read_u16_le(bytes, 2, "res entry")?;
    let key = read_u32_le(bytes, 4, "res entry")?;
    let value = if flags & FLAG_COMPLEX != 0 {
        let parent = read_u32_le(bytes, 8, "res entry parent")?;
        let count = read_u32_le(bytes, 12, "res entry count")? as usize;
        let entries = (0..count)
            .map(|i| {
                let at = COMPLEX_LEN + i * MAP_ENTRY_LEN;
                Ok(MapEntry {
                    name: read_u32_le(bytes, at, "map entry")?,
                    value: ResValue::new(bytes[at + 7], read_u32_le(bytes, at + 8, "map entry")?),
                })
            })
            .collect::<Result<_>>()?;
        EntryValue::Complex { parent, entries }
    } else {
        EntryValue::Simple(ResValue::new(
            bytes[11],
            read_u32_le(bytes, 12, "res value")?,
        ))
    };
    Ok(ResEntry { flags, key, value })
}

pub(super) fn serialize_entry(out: &mut Vec<u8>, entry: &ResEntry) {
    match &entry.value {
        EntryValue::Simple(value) => {
            write_u16(out, SIMPLE_LEN as u16);
            write_u16(out, entry.flags);
            write_u32(out, entry.key);
            write_value(out, *value);
        }
        EntryValue::Complex { parent, entries } => {
            write_u16(out, COMPLEX_LEN as u16);
            write_u16(out, entry.flags | FLAG_COMPLEX);
            write_u32(out, entry.key);
            write_u32(out, *parent);
            write_u32(out, entries.len() as u32);
            for map_entry in entries {
                write_u32(out, map_entry.name);
                write_value(out, map_entry.value);
            }
        }
    }
}

fn write_value(out: &mut Vec<u8>, value: ResValue) {
    write_u16(out, 8);
    out.push(0);
    out.push(value.kind);
    write_u32(out, value.data);
}
