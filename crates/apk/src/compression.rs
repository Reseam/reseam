// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufWriter, Read, Seek, Write};

use crate::Result;

/// Streams one DEX entry into a ZIP archive. Both native DEX workers and browser
/// compression workers use this function so ZIP metadata and DEFLATE agree.
pub fn compress_dex_entry<W: Write + Seek>(
    mut source: impl Read,
    destination: W,
    name: &str,
    level: i64,
) -> Result<W> {
    let mut archive = zip::ZipWriter::new(BufWriter::new(destination));
    archive.start_file(
        name,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(level)),
    )?;
    io::copy(&mut source, &mut archive)?;
    Ok(archive
        .finish()?
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?)
}
