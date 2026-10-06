// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{COMPLEX_LEN, EntryValue, FLAG_COMPACT, FLAG_COMPLEX, ResEntry, SIMPLE_LEN};
use crate::buf::{write_u16, write_u32};

pub(in crate::resources) fn serialize_entry(out: &mut Vec<u8>, entry: &ResEntry) {
    match &entry.value {
        EntryValue::Simple(value) => {
            write_u16(out, SIMPLE_LEN as u16);
            write_u16(out, entry.flags & !(FLAG_COMPLEX | FLAG_COMPACT));
            write_u32(out, entry.key);
            out.extend(value.encoded());
        }
        EntryValue::Complex { parent, entries } => {
            write_u16(out, COMPLEX_LEN as u16);
            write_u16(out, (entry.flags | FLAG_COMPLEX) & !FLAG_COMPACT);
            write_u32(out, entry.key);
            write_u32(out, *parent);
            write_u32(out, entries.len() as u32);
            for map_entry in entries {
                write_u32(out, map_entry.name);
                out.extend(map_entry.value.encoded());
            }
        }
    }
}
