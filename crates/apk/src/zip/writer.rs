// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use std::fs::File;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read, Seek, Write};

/// Alignment for most APK entries (4 bytes).
pub const ALIGNMENT_DEFAULT: u16 = 4;
/// Alignment for native libraries (16 KB page alignment).
pub const ALIGNMENT_NATIVE_LIB: u16 = 16 * 1024;

/// Writes an APK by selectively replacing entries while passing through unchanged ones.
pub struct ApkWriter<W: Write + Seek> {
    writer: zip::ZipWriter<W>,
}

pub struct ApkReplacement<'a> {
    pub data: ReplacementData<'a>,
    pub compression: zip::CompressionMethod,
}

/// Where a replacement entry's bytes come from.
pub enum ReplacementData<'a> {
    Bytes(&'a [u8]),
    File(&'a File),
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
        data: &ReplacementData<'_>,
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let alignment = entry_alignment(name);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .with_alignment(alignment);

        self.writer.start_file(name, options)?;
        match data {
            ReplacementData::Bytes(bytes) => self.writer.write_all(bytes)?,
            ReplacementData::File(file) => {
                let mut reader = io::BufReader::new(*file);
                reader.seek(io::SeekFrom::Start(0))?;
                io::copy(&mut reader, &mut self.writer)?;
            }
        }
        Ok(())
    }

    /// Write the complete APK by passing through all entries from source,
    /// replacing those in `replacements`, and removing those in `removals`.
    ///
    /// `dex_names` are the DEX entries this component receives; `compressed`
    /// yields each one as a single-entry archive whose compressed bytes are
    /// copied verbatim. Replacements and DEX entries missing from `source`
    /// are appended at the end.
    pub fn rewrite_apk<R: Read + Seek>(
        &mut self,
        source: &mut zip::ZipArchive<R>,
        replacements: &BTreeMap<&str, ApkReplacement<'_>>,
        removals: &HashSet<String>,
        dex_names: &[String],
        mut compressed: impl FnMut(&str) -> Result<Option<File>>,
    ) -> Result<()> {
        let mut written = HashSet::new();

        for i in 0..source.len() {
            let (name, compression) = {
                let entry = source.by_index_raw(i)?;
                (entry.name().to_string(), entry.compression())
            };

            if removals.contains(&name) {
                continue;
            }

            if let Some(archive) = compressed(&name)? {
                self.copy_compressed(archive)?;
                written.insert(name);
            } else if let Some(replacement) = replacements.get(name.as_str()) {
                self.write_entry(&name, &replacement.data, replacement.compression)?;
                written.insert(name);
            } else if should_rewrite_passthrough_for_alignment(compression) {
                let entry = source.by_index(i)?;
                self.copy_entry_with_alignment(entry)?;
            } else {
                let entry = source.by_index_raw(i)?;
                self.writer.raw_copy_file(entry)?;
            }
        }

        for (name, replacement) in replacements {
            if !written.contains(*name) {
                self.write_entry(name, &replacement.data, replacement.compression)?;
            }
        }
        for name in dex_names {
            if !written.contains(name) {
                let archive = compressed(name)?
                    .ok_or_else(|| crate::error::invalid("dex write", format!("{name} was not produced")))?;
                self.copy_compressed(archive)?;
            }
        }

        Ok(())
    }

    fn copy_compressed(&mut self, archive: File) -> Result<()> {
        let mut archive = zip::ZipArchive::new(io::BufReader::new(archive))?;
        self.writer.raw_copy_file(archive.by_index_raw(0)?)?;
        Ok(())
    }

    fn copy_entry_with_alignment(&mut self, mut entry: zip::read::ZipFile<'_>) -> Result<()> {
        let name = entry.name().to_string();
        let options = entry.options().with_alignment(entry_alignment(&name));

        self.writer.start_file(&name, options)?;
        io::copy(&mut entry, &mut self.writer)?;
        Ok(())
    }

    /// Finalize the ZIP (writes central directory). Returns the inner writer.
    pub fn finish(self) -> Result<W> {
        Ok(self.writer.finish()?)
    }
}

/// Determine the alignment for a ZIP entry based on its name.
fn entry_alignment(name: &str) -> u16 {
    if is_native_library_entry(name) {
        ALIGNMENT_NATIVE_LIB
    } else {
        ALIGNMENT_DEFAULT
    }
}

pub fn is_native_library_entry(name: &str) -> bool {
    let mut parts = name.split('/');
    matches!(parts.next(), Some("lib"))
        && parts.next().is_some()
        && parts
            .next()
            .is_some_and(|file_name| file_name.ends_with(".so"))
        && parts.next().is_none()
}

/// Only stored entries are mapped by the platform, so only they need
/// their alignment restored; deflated ones are copied compressed.
fn should_rewrite_passthrough_for_alignment(compression: zip::CompressionMethod) -> bool {
    compression == zip::CompressionMethod::Stored
}
