// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{self, BufReader, Read, Seek, SeekFrom, Take};

use flate2::Crc;
use flate2::bufread::DeflateDecoder;
use reseam_storage::file::FileReader;
use zip::CompressionMethod;

use super::reader::{self, Archive};
use crate::error::{Result, invalid};

/// The most compressed data read ahead of the inflater at once.
const READ_AHEAD: u64 = 64 << 10;

/// The uncompressed bytes of an entry of the input archive, decoded as they are
/// read. Reaching the end checks them against the size and CRC-32 the central
/// directory records.
pub(crate) struct EntryData {
    decoder: Decoder,
    crc: Crc,
    read: u64,
    size: u64,
    expected_crc: u32,
}

enum Decoder {
    Stored(Take<FileReader>),
    Deflated(DeflateDecoder<BufReader<Take<FileReader>>>),
}

impl EntryData {
    /// Positions a reader of its own at entry `name`'s data, so the result
    /// outlives `archive`'s borrow.
    pub(crate) fn open(archive: &mut Archive, name: &str) -> Result<Self> {
        let entry = archive.by_index_raw(reader::index_of(archive, name)?)?;
        if entry.encrypted() {
            return Err(invalid("zip entry", format!("{name}: entry is encrypted")));
        }
        let start = reader::data_start(&entry, name)?;
        let (method, compressed) = (entry.compression(), entry.compressed_size());
        let (size, expected_crc) = (entry.size(), entry.crc32());
        drop(entry);
        let mut file = archive.clone().into_inner();
        file.seek(SeekFrom::Start(start))?;
        let data = file.take(compressed);
        let decoder = match method {
            CompressionMethod::Stored => Decoder::Stored(data),
            CompressionMethod::Deflated => Decoder::Deflated(DeflateDecoder::new(
                BufReader::with_capacity(compressed.clamp(1, READ_AHEAD) as usize, data),
            )),
            other => {
                return Err(invalid(
                    "zip entry",
                    format!("{name}: unsupported compression method {other}"),
                ));
            }
        };
        Ok(Self {
            decoder,
            crc: Crc::new(),
            read: 0,
            size,
            expected_crc,
        })
    }

    fn check_end(&self) -> io::Result<()> {
        let mismatch = if self.read != self.size {
            format!("entry ended after {} of {} bytes", self.read, self.size)
        } else if self.crc.sum() != self.expected_crc {
            "entry data does not match its CRC-32".to_owned()
        } else {
            return Ok(());
        };
        Err(io::Error::new(io::ErrorKind::InvalidData, mismatch))
    }
}

impl Read for EntryData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = match &mut self.decoder {
            Decoder::Stored(data) => data.read(buf)?,
            Decoder::Deflated(data) => data.read(buf)?,
        };
        if count == 0 && !buf.is_empty() {
            self.check_end()?;
        }
        self.read += count as u64;
        if self.read > self.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("entry holds more than its recorded {} bytes", self.size),
            ));
        }
        self.crc.update(&buf[..count]);
        Ok(count)
    }
}
