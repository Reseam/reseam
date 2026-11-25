// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The library's loaded data, where the strings code references lie.

use std::ops::Range;

use crate::error::Result;
use crate::file::ElfImage;

/// Where the library's loaded data lies: the addresses of the strings code can reference.
pub(crate) struct LoadedData(Vec<Range<u64>>);

impl LoadedData {
    pub(crate) fn contains(&self, address: u64) -> bool {
        self.0.iter().any(|range| range.contains(&address))
    }
}

impl ElfImage {
    pub(crate) fn loaded_data(&self) -> LoadedData {
        LoadedData(
            self.sections()
                .iter()
                .filter(|section| section.is_data())
                .map(|section| section.address..section.address + section.size)
                .collect(),
        )
    }
}

/// Every address in loaded data that holds `text`.
pub(crate) fn data_addresses(image: &ElfImage, text: &[u8]) -> Result<Vec<u64>> {
    let mut addresses = Vec::new();
    for section in image.sections().iter().filter(|section| section.is_data()) {
        let bytes = image.contents(section)?;
        addresses.extend(
            memchr::memmem::find_iter(bytes, text)
                .map(|offset| section.address.wrapping_add(offset as u64)),
        );
    }
    Ok(addresses)
}
