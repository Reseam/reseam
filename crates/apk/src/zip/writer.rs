// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufWriter, Write};

use crate::entry::is_native_library;
use crate::error::Result;
use crate::zip::reader::{self, Archive};

pub(crate) type Writer = zip::ZipWriter<BufWriter<File>>;

pub(crate) fn copy_original(writer: &mut Writer, source: &mut Archive, index: usize) -> Result<()> {
    let entry = source.by_index_raw(index)?;
    if entry.compression() == zip::CompressionMethod::Stored {
        let name = entry.name().to_string();
        let options = entry.options().with_alignment(entry_alignment(&name));
        writer.start_file(&name, options)?;
        drop(entry);
        io::copy(&mut source.by_index(index)?, writer)?;
    } else {
        writer.raw_copy_file(entry)?;
    }
    Ok(())
}

pub(crate) fn write_bytes(
    writer: &mut Writer,
    name: &str,
    bytes: &[u8],
    compression: zip::CompressionMethod,
) -> Result<()> {
    writer.start_file(name, options(name, compression))?;
    writer.write_all(bytes)?;
    Ok(())
}

pub(crate) fn write_file(
    writer: &mut Writer,
    name: &str,
    file: &File,
    compression: zip::CompressionMethod,
) -> Result<()> {
    writer.start_file(name, options(name, compression))?;
    reader::copy_spooled(file, writer)
}

pub(crate) fn copy_compressed(writer: &mut Writer, file: File) -> Result<()> {
    let mut archive = zip::ZipArchive::new(io::BufReader::new(file))?;
    writer.raw_copy_file(archive.by_index_raw(0)?)?;
    Ok(())
}

fn options(name: &str, compression: zip::CompressionMethod) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default()
        .compression_method(compression)
        .with_alignment(entry_alignment(name))
}

fn entry_alignment(name: &str) -> u16 {
    if is_native_library(name) {
        16 * 1024
    } else {
        4
    }
}
