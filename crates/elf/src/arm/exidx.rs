// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::u32_at;

use crate::error::{Result, invalid};
use crate::file::ElfImage;

/// The function starts the `.ARM.exidx` unwind table holds, sorted. Each entry starts with a
/// 31-bit offset from itself to its function, and the table has no sizes. Linkers merge
/// consecutive entries that unwind alike, so a function may have no entry of its own; a library
/// built without unwind tables has none.
pub(super) fn function_starts(image: &ElfImage) -> Result<Vec<u64>> {
    let Some(section) = image.find_section(".ARM.exidx") else {
        return Ok(Vec::new());
    };
    image
        .contents(section)?
        .as_chunks::<8>()
        .0
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let word = u32_at(entry, 0);
            if word & 0x8000_0000 != 0 {
                return Err(invalid(
                    index * 8,
                    "unwind index entry without a function offset",
                ));
            }
            let offset = i64::from(((word << 1) as i32) >> 1);
            Ok(section
                .address
                .wrapping_add(index as u64 * 8)
                .wrapping_add_signed(offset))
        })
        .collect()
}
