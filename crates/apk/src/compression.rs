// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;

use crate::Result;
use crate::zip::Compression;
use crate::zip::writer::Writer;

/// Writes `source` as the single deflated entry `name` of a ZIP at
/// `destination`. Native DEX workers and the browser's compression workers
/// both use it, so the entries they hand the APK writer agree.
pub fn compress_dex_entry(
    source: &File,
    destination: File,
    name: &str,
    level: u32,
) -> Result<File> {
    let mut archive = Writer::new(destination, level);
    archive.write(name, source, Compression::Deflated)?;
    archive.finish()
}
