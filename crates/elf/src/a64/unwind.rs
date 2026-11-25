// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Function bounds from the unwind tables. Stripped libraries keep `.eh_frame_hdr`: a table of
//! every function's start, sorted, each pointing at a frame description entry holding its size.

use reseam_bytes::{i32_at, read_u8, read_u32};

use crate::dwarf::Reader;
use crate::error::{Result, invalid};
use crate::file::ElfImage;
use crate::{Function, InstructionSet};

const HEADER_VERSION: u8 = 1;
/// A signed four-byte value relative to the start of `.eh_frame_hdr`, the binary search table's
/// encoding.
const TABLE_ENCODING: u8 = 0x3b;
const OMIT: u8 = 0xff;

/// Every function with unwind information, sorted by start address: the search table in
/// `.eh_frame_hdr`, read in place.
pub(crate) struct FunctionTable {
    /// The address of `.eh_frame_hdr`, which the table's values are relative to.
    header: u64,
    /// The file offset of the table.
    table: usize,
    count: usize,
}

impl FunctionTable {
    pub(crate) fn read(image: &ElfImage) -> Result<Self> {
        let header = image.section(".eh_frame_hdr")?;
        let bytes = image.contents(header)?;
        if read_u8(bytes, 0, "unwind table header")? != HEADER_VERSION {
            return Err(invalid(0, "unsupported .eh_frame_hdr version"));
        }
        let pointer_encoding = read_u8(bytes, 1, "unwind table header")?;
        let count_encoding = read_u8(bytes, 2, "unwind table header")?;
        if read_u8(bytes, 3, "unwind table header")? != TABLE_ENCODING || count_encoding == OMIT {
            return Err(invalid(
                3,
                "unsupported .eh_frame_hdr search table encoding",
            ));
        }
        let mut reader = Reader::new(bytes, header.address, 4);
        reader.pointer(pointer_encoding)?;
        let count = reader.pointer(count_encoding)?;
        let table = reader.position;
        let count = usize::try_from(count)
            .ok()
            .filter(|count| count.checked_mul(8).is_some())
            .ok_or_else(|| invalid(table, "unwind search table past the address space"))?;
        reseam_bytes::require_len(bytes, table, count * 8, "unwind search table")?;
        Ok(Self {
            header: header.address,
            table: image.offset(header.address + table as u64, 0)?,
            count,
        })
    }

    /// The table's entries: a function's start and the address of its frame description entry,
    /// four bytes each.
    fn entries<'a>(&self, image: &'a ElfImage) -> &'a [[u8; 8]] {
        &image.bytes()[self.table..].as_chunks::<8>().0[..self.count]
    }

    fn start(&self, entry: &[u8]) -> u64 {
        relative(self.header, i32_at(entry, 0))
    }

    pub(super) fn starts<'a>(&'a self, image: &'a ElfImage) -> impl Iterator<Item = u64> + 'a {
        self.entries(image).iter().map(|entry| self.start(entry))
    }

    /// The function whose code holds `address`, when unwind information covers it.
    pub(crate) fn containing(&self, image: &ElfImage, address: u64) -> Result<Option<Function>> {
        let entries = self.entries(image);
        let below = entries.partition_point(|entry| self.start(entry) <= address);
        let Some(entry) = below.checked_sub(1).map(|index| &entries[index]) else {
            return Ok(None);
        };
        let function = frame_description(image, relative(self.header, i32_at(entry, 4)))?;
        Ok(function.contains(address).then_some(function))
    }
}

fn relative(base: u64, offset: i32) -> u64 {
    base.wrapping_add_signed(i64::from(offset))
}

/// The function a frame description entry at `address` in `.eh_frame` covers.
fn frame_description(image: &ElfImage, address: u64) -> Result<Function> {
    let frames = image.section(".eh_frame")?;
    let bytes = image.contents(frames)?;
    let at = address
        .checked_sub(frames.address)
        .filter(|&at| at < frames.size)
        .ok_or_else(|| invalid(0, "frame description entry outside .eh_frame"))?
        as usize;
    let length = read_u32(bytes, at, "frame description entry")?;
    if length == u32::MAX {
        return Err(invalid(
            at,
            "64-bit frame description entries are unsupported",
        ));
    }
    let cie_pointer = read_u32(bytes, at + 4, "frame description entry")? as usize;
    let cie = (at + 4)
        .checked_sub(cie_pointer)
        .ok_or_else(|| invalid(at, "frame description entry points before .eh_frame"))?;
    let encoding = pointer_encoding(bytes, cie)?;
    let mut reader = Reader::new(bytes, frames.address, at + 8);
    let start = reader.pointer(encoding)?;
    let size = reader.pointer(encoding & 0x0f)?;
    Ok(Function {
        address: start,
        size,
        instruction_set: InstructionSet::A64,
    })
}

/// The pointer encoding a common information entry's `R` augmentation declares.
fn pointer_encoding(bytes: &[u8], cie: usize) -> Result<u8> {
    let mut reader = Reader::new(bytes, 0, cie + 8);
    let version = reader.u8()?;
    let augmentation_start = reader.position;
    let end = memchr::memchr(0, &bytes[augmentation_start.min(bytes.len())..])
        .ok_or_else(|| invalid(augmentation_start, "unterminated CIE augmentation"))?;
    let augmentation = &bytes[augmentation_start..augmentation_start + end];
    reader.position = augmentation_start + end + 1;
    if version >= 4 {
        reader.position += 2;
    }
    reader.uleb()?;
    reader.sleb()?;
    if version == 1 {
        reader.u8()?;
    } else {
        reader.uleb()?;
    }
    let Some(rest) = augmentation.strip_prefix(b"z") else {
        return Ok(0);
    };
    reader.uleb()?;
    for &kind in rest {
        match kind {
            b'R' => return reader.u8(),
            b'P' => {
                let encoding = reader.u8()?;
                reader.pointer(encoding)?;
            }
            b'L' => {
                reader.u8()?;
            }
            b'S' | b'B' => {}
            _ => return Err(invalid(augmentation_start, "unknown CIE augmentation")),
        }
    }
    Ok(0)
}
