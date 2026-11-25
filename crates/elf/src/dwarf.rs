// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading DWARF values: LEB128 integers, which Android's packed relocations use too, and the
//! `.eh_frame` pointer encodings.

use reseam_bytes::read_u8;

use crate::error::{Result, invalid};

/// Reads DWARF exception-handling values from a section loaded at `address`.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) address: u64,
    pub(crate) position: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8], address: u64, position: usize) -> Self {
        Self {
            bytes,
            address,
            position,
        }
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        let value = read_u8(self.bytes, self.position, "unwind information")?;
        self.position += 1;
        Ok(value)
    }

    pub(crate) fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = reseam_bytes::slice(self.bytes, self.position, N, "unwind information")?;
        self.position += N;
        Ok(bytes.try_into().expect("slice returned N bytes"))
    }

    pub(crate) fn uleb(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(invalid(self.position, "LEB128 value past 64 bits"))
    }

    pub(crate) fn sleb(&mut self) -> Result<i64> {
        let mut value = 0i64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            value |= i64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if shift + 7 < 64 && byte & 0x40 != 0 {
                    value |= -1i64 << (shift + 7);
                }
                return Ok(value);
            }
        }
        Err(invalid(self.position, "LEB128 value past 64 bits"))
    }

    /// A pointer in `encoding`: its value format in the low nibble, what it is relative to in the
    /// high one.
    pub(crate) fn pointer(&mut self, encoding: u8) -> Result<u64> {
        let at = self.address.wrapping_add(self.position as u64);
        let value = match encoding & 0x0f {
            0x00 | 0x04 => u64::from_le_bytes(self.fixed()?),
            0x01 => self.uleb()?,
            0x02 => u64::from(u16::from_le_bytes(self.fixed()?)),
            0x03 => u64::from(u32::from_le_bytes(self.fixed()?)),
            0x09 => self.sleb()? as u64,
            0x0a => i64::from(i16::from_le_bytes(self.fixed()?)) as u64,
            0x0b => i64::from(i32::from_le_bytes(self.fixed()?)) as u64,
            0x0c => i64::from_le_bytes(self.fixed()?) as u64,
            _ => return Err(invalid(self.position, "unknown pointer encoding")),
        };
        Ok(match encoding & 0x70 {
            0x00 => value,
            0x10 => at.wrapping_add(value),
            0x30 => self.address.wrapping_add(value),
            _ => return Err(invalid(self.position, "unsupported pointer application")),
        })
    }
}
