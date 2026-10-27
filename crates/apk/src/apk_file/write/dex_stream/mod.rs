// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rewritten DEX entries produced off the writing thread, in job order: by
//! worker threads natively, by the host's compression workers in a browser.

#[cfg(not(target_os = "wasi"))]
mod native;
#[cfg(target_os = "wasi")]
mod wasi;

#[cfg(not(target_os = "wasi"))]
pub(super) use native::DexEntryStream;
#[cfg(target_os = "wasi")]
pub(super) use wasi::DexEntryStream;

use std::fs::File;
use std::io::Read;

use reseam_dex::{Spooled, WriteLayout};

use crate::error::Result;

/// A rewritten DEX ready to be added to its APK, with where each of its
/// members' pools and classes landed.
pub(super) struct WrittenDex {
    pub entry: DexEntry,
    pub layouts: Vec<WriteLayout>,
}

pub(super) enum DexEntry {
    /// A single-entry ZIP holding the deflated DEX.
    Deflated(File),
    /// The serialized DEX and its CRC-32.
    Stored { file: File, crc: u32 },
}

/// The entry for a stored job, which needs no compression worker. `buffer`
/// carries the bytes being checksummed.
fn stored_entry(spooled: Spooled, buffer: &mut [u8]) -> Result<DexEntry> {
    let mut crc = flate2::Crc::new();
    let mut reader = spooled.reader();
    loop {
        let read = reader.read(buffer)?;
        if read == 0 {
            break;
        }
        crc.update(&buffer[..read]);
    }
    drop(reader);
    Ok(DexEntry::Stored {
        file: spooled.into_file(),
        crc: crc.sum(),
    })
}
