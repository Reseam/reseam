// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{read_u16, read_u32, slice};
use std::io::Read;

use super::SECTION;
use crate::error::{Result, invalid, malformed};

/// Profile fields read in order, little-endian.
pub(super) struct Cursor<'a> {
    buf: &'a [u8],
    pub(super) pos: usize,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(super) fn is_at_end(&self) -> bool {
        self.pos == self.buf.len()
    }

    pub(super) fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let bytes = slice(self.buf, self.pos, len, SECTION)?;
        self.pos += len;
        Ok(bytes)
    }

    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16> {
        let value = read_u16(self.buf, self.pos, SECTION)?;
        self.pos += 2;
        Ok(value)
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        let value = read_u32(self.buf, self.pos, SECTION)?;
        self.pos += 4;
        Ok(value)
    }

    pub(super) fn string(&mut self, len: usize) -> Result<String> {
        let at = self.pos;
        String::from_utf8(self.bytes(len)?.to_vec())
            .map_err(|_| malformed(SECTION, at, "profile key is not UTF-8"))
    }

    pub(super) fn ascending(&mut self, count: usize) -> Result<Vec<u32>> {
        let mut last = 0u32;
        (0..count)
            .map(|_| {
                last += u32::from(self.u16()?);
                Ok(last)
            })
            .collect()
    }

    /// The zlib block that follows a `count, inflated size, deflated size` header.
    pub(super) fn inflated(&mut self, inflated: u32, deflated: u32) -> Result<Vec<u8>> {
        let at = self.pos;
        let mut body = Vec::with_capacity(inflated as usize);
        flate2::read::ZlibDecoder::new(self.bytes(deflated as usize)?).read_to_end(&mut body)?;
        if body.len() != inflated as usize || self.pos != self.buf.len() {
            return Err(malformed(SECTION, at, "compressed body size mismatch"));
        }
        Ok(body)
    }

    pub(super) fn header(&mut self, magic: [u8; 4], version: [u8; 4]) -> Result<()> {
        if self.bytes(4)? != magic {
            return Err(malformed(SECTION, 0, "bad magic"));
        }
        let found = self.bytes(4)?;
        if found != version {
            return Err(invalid(
                SECTION,
                format!("unsupported version {}", String::from_utf8_lossy(found)),
            ));
        }
        Ok(())
    }
}
