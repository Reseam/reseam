// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Seek, Write};

/// Alignment for most APK entries (4 bytes).
pub const ALIGNMENT_DEFAULT: u16 = 4;
/// Alignment for native libraries (4KB page alignment).
pub const ALIGNMENT_NATIVE_LIB: u16 = 4096;

/// Writes an APK by selectively replacing entries while passing through unchanged ones.
pub struct ApkWriter<W: Write + Seek> {
    writer: zip::ZipWriter<W>,
}

pub struct ApkReplacement<'a> {
    pub data: &'a [u8],
    pub compression: zip::CompressionMethod,
}

impl<W: Write + Seek> ApkWriter<W> {
    pub fn new(dest: W) -> Self {
        Self {
            writer: zip::ZipWriter::new(dest),
        }
    }

    /// Write a new or replacement entry with proper alignment.
    pub fn write_entry(
        &mut self,
        name: &str,
        data: &[u8],
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let alignment = entry_alignment(name);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .with_alignment(alignment);

        self.writer.start_file(name, options)?;
        self.writer.write_all(data)?;
        Ok(())
    }

    /// Write the complete APK by passing through all entries from source,
    /// replacing those in `replacements`, and removing those in `removals`.
    ///
    /// Entries in `replacements` that don't exist in `source` are appended at the end.
    pub fn rewrite_apk<R: Read + Seek>(
        &mut self,
        source: &mut zip::ZipArchive<R>,
        replacements: &BTreeMap<&str, ApkReplacement<'_>>,
        removals: &HashSet<String>,
    ) -> Result<()> {
        // Track which replacement entries we've written (so we can append new ones at the end)
        let mut written_replacements = HashSet::new();

        for i in 0..source.len() {
            // Get the entry name (need to borrow and release before doing anything else)
            let name = {
                let entry = source.by_index_raw(i)?;
                entry.name().to_string()
            };

            if removals.contains(&name) {
                continue;
            }

            if let Some(replacement) = replacements.get(name.as_str()) {
                self.write_entry(&name, replacement.data, replacement.compression)?;
                written_replacements.insert(name);
            } else if needs_stored_alignment(&name) {
                // resources.arsc must be stored uncompressed + aligned (Android API 30+)
                let mut entry = source.by_index(i)?;
                let mut data = Vec::new();
                std::io::copy(&mut entry, &mut data)?;
                drop(entry);
                self.write_entry(&name, &data, zip::CompressionMethod::Stored)?;
            } else {
                // Pass-through: copy compressed bytes as-is
                let entry = source.by_index_raw(i)?;
                self.writer.raw_copy_file(entry)?;
            }
        }

        // Append any replacement entries that didn't exist in the source
        for (name, replacement) in replacements {
            if !written_replacements.contains(*name) {
                self.write_entry(name, replacement.data, replacement.compression)?;
            }
        }

        Ok(())
    }

    /// Finalize the ZIP (writes central directory). Returns the inner writer.
    pub fn finish(self) -> Result<W> {
        Ok(self.writer.finish()?)
    }
}

/// Determine the alignment for a ZIP entry based on its name.
fn entry_alignment(name: &str) -> u16 {
    if name.ends_with(".so") {
        ALIGNMENT_NATIVE_LIB
    } else {
        ALIGNMENT_DEFAULT
    }
}

/// Whether an entry must be stored uncompressed + aligned regardless of source compression.
fn needs_stored_alignment(name: &str) -> bool {
    name == "resources.arsc"
}
