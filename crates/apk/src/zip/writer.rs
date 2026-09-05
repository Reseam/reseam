// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Seek, Write};

use crate::entry::is_native_library;
use crate::error::{invalid, Result};

const ALIGNMENT_DEFAULT: u16 = 4;
const ALIGNMENT_NATIVE_LIB: u16 = 16 * 1024;

pub(crate) struct ApkWriter<W: Write + Seek> {
    writer: zip::ZipWriter<W>,
}

pub(crate) struct Replacement<'a> {
    pub data: ReplacementData<'a>,
    pub compression: zip::CompressionMethod,
}

pub(crate) enum ReplacementData<'a> {
    Bytes(&'a [u8]),
    File(&'a File),
}

impl<W: Write + Seek> ApkWriter<W> {
    pub fn new(dest: W) -> Self {
        Self {
            writer: zip::ZipWriter::new(dest),
        }
    }

    /// Copies every entry of `source` except `removals`, substituting
    /// `replacements`, and appends replacements and DEX entries that were not
    /// in the source. `dex_names` are the DEX entries this archive receives;
    /// `compressed` yields each as a single-entry archive whose deflated
    /// bytes are copied verbatim.
    pub fn rewrite<R: Read + Seek>(
        &mut self,
        source: &mut zip::ZipArchive<R>,
        replacements: &BTreeMap<&str, Replacement<'_>>,
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
            if removals.contains(name.as_str()) {
                continue;
            }
            if let Some(archive) = compressed(&name)? {
                self.copy_compressed(archive)?;
                written.insert(name);
            } else if let Some(replacement) = replacements.get(name.as_str()) {
                self.write_entry(&name, &replacement.data, replacement.compression)?;
                written.insert(name);
            } else if compression == zip::CompressionMethod::Stored {
                // Only stored entries are mapped by the platform, so only they
                // need their alignment restored.
                self.copy_aligned(source.by_index(i)?)?;
            } else {
                self.writer.raw_copy_file(source.by_index_raw(i)?)?;
            }
        }
        for (name, replacement) in replacements {
            if !written.contains(*name) {
                self.write_entry(name, &replacement.data, replacement.compression)?;
            }
        }
        for name in dex_names {
            if !written.contains(name.as_str()) {
                let archive = compressed(name.as_str())?
                    .ok_or_else(|| invalid("dex write", format!("{name} was not produced")))?;
                self.copy_compressed(archive)?;
            }
        }
        Ok(())
    }

    fn write_entry(
        &mut self,
        name: &str,
        data: &ReplacementData<'_>,
        compression: zip::CompressionMethod,
    ) -> Result<()> {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .with_alignment(entry_alignment(name));
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

    fn copy_compressed(&mut self, archive: File) -> Result<()> {
        let mut archive = zip::ZipArchive::new(io::BufReader::new(archive))?;
        self.writer.raw_copy_file(archive.by_index_raw(0)?)?;
        Ok(())
    }

    fn copy_aligned(&mut self, mut entry: zip::read::ZipFile<'_>) -> Result<()> {
        let name = entry.name().to_string();
        let options = entry.options().with_alignment(entry_alignment(&name));
        self.writer.start_file(&name, options)?;
        io::copy(&mut entry, &mut self.writer)?;
        Ok(())
    }

    pub fn finish(self) -> Result<W> {
        Ok(self.writer.finish()?)
    }
}

fn entry_alignment(name: &str) -> u16 {
    if is_native_library(name) {
        ALIGNMENT_NATIVE_LIB
    } else {
        ALIGNMENT_DEFAULT
    }
}
