// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The words the dynamic linker moves by the library's load address. A library stores pointers to
//! its own code this way, in vtables, callback tables and `.init_array`. The pointer itself is the
//! word already at the location, the relocation's implicit addend.

use reseam_bytes::u32_at;

use crate::dwarf::Reader;
use crate::error::{Result, invalid};
use crate::file::ElfImage;

const SECTION_REL: u32 = 9;
const SECTION_RELR: u32 = 19;
/// Android's packed form of REL, `APS2` followed by SLEB128 groups.
const SECTION_ANDROID_REL: u32 = 0x6000_0001;
/// RELR under the type Android used before the standard one.
const SECTION_ANDROID_RELR: u32 = 0x6fff_ff00;
const R_ARM_RELATIVE: u32 = 23;

/// Packed group flags, as Android's linker reads them.
const GROUPED_BY_INFO: i64 = 1;
const GROUPED_BY_OFFSET_DELTA: i64 = 2;
const GROUPED_BY_ADDEND: i64 = 4;
const GROUP_HAS_ADDEND: i64 = 8;

/// The values of every word relocated by the load address.
pub(super) fn relative_values(image: &ElfImage) -> Result<Vec<u64>> {
    let mut locations = Vec::new();
    for section in image.sections() {
        let bytes = || image.contents(section);
        match section.kind {
            SECTION_REL => locations.extend(rel(bytes()?)),
            SECTION_ANDROID_REL => locations.extend(packed(bytes()?)?),
            SECTION_RELR | SECTION_ANDROID_RELR => locations.extend(relr(bytes()?)),
            _ => {}
        }
    }
    locations
        .into_iter()
        .map(|location| Ok(u64::from(u32_at(image.loaded(location, 4)?, 0))))
        .collect()
}

fn is_relative(info: u32) -> bool {
    info & 0xff == R_ARM_RELATIVE
}

/// REL entries: an offset and an info word, whose low byte is the type.
fn rel(bytes: &[u8]) -> impl Iterator<Item = u64> + '_ {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .filter(|entry| is_relative(u32_at(&entry[..], 4)))
        .map(|entry| u64::from(u32_at(&entry[..], 0)))
}

/// RELR: an even word is a location, and an odd one a bitmap of the 31 words after the last
/// location, bit `n` standing for word `n - 1`.
fn relr(bytes: &[u8]) -> Vec<u64> {
    let mut locations = Vec::new();
    let mut next = 0u64;
    for &word in bytes.as_chunks::<4>().0 {
        let word = u64::from(u32::from_le_bytes(word));
        if word & 1 == 0 {
            locations.push(word);
            next = word + 4;
        } else {
            locations.extend(
                (1..32)
                    .filter(|bit| (word >> bit) & 1 == 1)
                    .map(|bit| next + (bit - 1) * 4),
            );
            next += 31 * 4;
        }
    }
    locations
}

/// Android's packed relocations: a count and a start offset, then groups that share an offset
/// step, an info word or both. REL relocations carry no addends, so a group with one is invalid.
fn packed(bytes: &[u8]) -> Result<Vec<u64>> {
    if !bytes.starts_with(b"APS2") {
        return Err(invalid(0, "packed relocations without the APS2 magic"));
    }
    let mut reader = Reader::new(bytes, 0, 4);
    let count = reader.sleb()?;
    let mut offset = reader.sleb()?;
    let mut info = 0;
    let mut locations = Vec::new();
    let mut read = 0;
    while read < count {
        let size = reader.sleb()?;
        if size <= 0 {
            return Err(invalid(reader.position, "empty packed relocation group"));
        }
        let flags = reader.sleb()?;
        if flags & (GROUP_HAS_ADDEND | GROUPED_BY_ADDEND) != 0 {
            return Err(invalid(
                reader.position,
                "addends in packed REL relocations",
            ));
        }
        let delta = if flags & GROUPED_BY_OFFSET_DELTA != 0 {
            Some(reader.sleb()?)
        } else {
            None
        };
        if flags & GROUPED_BY_INFO != 0 {
            info = reader.sleb()?;
        }
        for _ in 0..size {
            offset = offset.wrapping_add(match delta {
                Some(delta) => delta,
                None => reader.sleb()?,
            });
            if flags & GROUPED_BY_INFO == 0 {
                info = reader.sleb()?;
            }
            if is_relative(info as u32) {
                locations.push(u64::from(offset as u32));
            }
        }
        read += size;
    }
    Ok(locations)
}
