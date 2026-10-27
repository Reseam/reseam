// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod deflate;
mod input;
mod output;
mod records;

pub(crate) use input::Source;

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

use flate2::Crc;

use crate::entry::is_native_library;
use crate::error::{Result, invalid};
use crate::zip::{COPY_BUFFER, Compression};
use output::Output;
use records::{EntryHeader, zip32};

const OUTPUT_BUFFER: usize = 64 << 10;
/// zlib's default level, for entries other than rewritten DEX.
pub(crate) const DEFAULT_LEVEL: u32 = 6;

/// A ZIP being written: an APK, or a single-entry archive. Entry data moves
/// between files in the kernel where the platform allows; the central
/// directory keeps one name and a few fields per entry.
pub(crate) struct Writer<W: Write + Seek> {
    out: Output<W>,
    level: u32,
    buffer: Box<[u8]>,
    entries: Vec<Entry>,
    header: Vec<u8>,
}

struct Entry {
    name: Box<str>,
    header: EntryHeader,
    offset: u32,
}

impl<W: Write + Seek> Writer<W> {
    /// A writer deflating the entries it is given uncompressed at `level`.
    pub(crate) fn new(output: W, level: u32) -> Self {
        Self {
            out: Output {
                file: BufWriter::with_capacity(OUTPUT_BUFFER, output),
                position: 0,
            },
            level,
            buffer: vec![0; COPY_BUFFER].into_boxed_slice(),
            entries: Vec::new(),
            header: Vec::new(),
        }
    }

    /// Copies entry `index` of `archive`, whose bytes `source` reads, as the
    /// archive holds it. Stored data is realigned.
    pub(crate) fn copy_original<R: Read + Seek>(
        &mut self,
        archive: &mut zip::ZipArchive<R>,
        source: &File,
        index: usize,
    ) -> Result<()> {
        let entry = archive.by_index_raw(index)?;
        let name: Box<str> = entry.name().into();
        let header = EntryHeader::of(&entry)?;
        let data = entry
            .data_start()
            .ok_or_else(|| invalid("zip entry", format!("{name}: missing data offset")))?;
        drop(entry);
        self.begin(name, header)?;
        self.copy_range(source, data, u64::from(header.compressed))
    }

    /// Copies the single entry of a ZIP a DEX worker wrote, returning its CRC-32.
    pub(crate) fn copy_compressed(&mut self, file: &File) -> Result<u32> {
        let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
        let crc = archive.by_index_raw(0)?.crc32();
        self.copy_original(&mut archive, file, 0)?;
        Ok(crc)
    }

    /// Adds a stored entry whose data and CRC-32 are already known.
    pub(crate) fn write_stored(&mut self, name: &str, file: &File, crc: u32) -> Result<()> {
        let len = file.metadata()?.len();
        let (compressed, size) = EntryHeader::sizes(len, len)?;
        self.begin(
            name.into(),
            EntryHeader::new(Compression::Stored, crc, compressed, size),
        )?;
        self.copy_range(file, 0, len)
    }

    /// Writes an entry whose CRC-32 and compressed size are known only once
    /// its data is written, then completes its local header.
    pub(crate) fn write(
        &mut self,
        name: &str,
        source: &(impl Source + ?Sized),
        compression: Compression,
    ) -> Result<()> {
        let len = source.len()?;
        let offset = self.begin(name.into(), EntryHeader::new(compression, 0, 0, 0))?;
        let start = self.out.position;
        let crc = match compression {
            Compression::Stored => self.copy_checksummed(source, len)?,
            Compression::Deflated => deflate::deflate(source, len, self.level, &mut self.out)?,
        };
        let (compressed, size) = EntryHeader::sizes(self.out.position - start, len)?;
        let header = &mut self.entries.last_mut().expect("the entry was begun").header;
        header.crc = crc.sum();
        header.compressed = compressed;
        header.size = size;
        let mut checksum = Vec::with_capacity(12);
        header.put_checksum(&mut checksum);
        self.out
            .patch(offset + EntryHeader::CHECKSUM_OFFSET, &checksum)
    }

    fn copy_checksummed(&mut self, source: &(impl Source + ?Sized), len: u64) -> Result<Crc> {
        let mut crc = Crc::new();
        let mut offset = 0;
        while offset < len {
            let count = (len - offset).min(self.buffer.len() as u64) as usize;
            let data = &mut self.buffer[..count];
            source.read_into(data, offset)?;
            crc.update(data);
            self.out.write_all(data)?;
            offset += count as u64;
        }
        Ok(crc)
    }

    pub(crate) fn finish(mut self) -> Result<W> {
        let start = self.out.position;
        for entry in &self.entries {
            self.header.clear();
            entry
                .header
                .central(&entry.name, entry.offset, &mut self.header);
            self.out.write_all(&self.header)?;
        }
        self.header.clear();
        records::end_of_central_directory(
            self.entries.len(),
            self.out.position - start,
            start,
            &mut self.header,
        )?;
        self.out.write_all(&self.header)?;
        Ok(self
            .out
            .file
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?)
    }

    /// Appends `len` bytes of `source` from `offset`. Data that fits the
    /// buffer joins the buffered output; larger data is copied by the kernel
    /// where the platform allows, which costs a flush and a few system calls.
    fn copy_range(&mut self, mut source: &File, offset: u64, len: u64) -> Result<()> {
        if let Some(data) = usize::try_from(len)
            .ok()
            .and_then(|len| self.buffer.get_mut(..len))
        {
            source.read_into(data, offset)?;
            self.out.file.write_all(data)?;
        } else {
            source.seek(SeekFrom::Start(offset))?;
            let copied = io::copy(&mut source.take(len), &mut self.out.file)?;
            if copied != len {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
        }
        self.out.position += len;
        Ok(())
    }

    /// Writes the local header of a new entry, returning where it starts.
    fn begin(&mut self, name: Box<str>, header: EntryHeader) -> Result<u64> {
        let offset = self.out.position;
        let alignment = match header.method {
            Compression::Stored => entry_alignment(&name),
            Compression::Deflated => 1,
        };
        header.local(&name, offset, alignment, &mut self.header);
        self.out.write_all(&self.header)?;
        self.entries.push(Entry {
            name,
            header,
            offset: zip32(offset, "entry offset")?,
        });
        Ok(offset)
    }
}

/// Stored native libraries are page-aligned so they load in place; other
/// stored entries are word-aligned.
fn entry_alignment(name: &str) -> u16 {
    if is_native_library(name) {
        16 * 1024
    } else {
        4
    }
}
