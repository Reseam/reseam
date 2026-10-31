// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The APK Signing Block: a size, ID-value pairs, the size again, and a magic.

use crate::error::{Result, SignError, invalid};

const MAGIC: &[u8; 16] = b"APK Sig Block 42";
pub(crate) const SIZE_LEN: usize = 8;
/// The trailing size field and magic.
pub(crate) const FOOTER_LEN: usize = SIZE_LEN + MAGIC.len();
/// A pair's length field and ID.
const PAIR_HEADER_LEN: usize = SIZE_LEN + 4;

pub(crate) const BLOCK_ID_V2: u32 = 0x7109_871a;
pub(crate) const BLOCK_ID_V3: u32 = 0xf053_68c0;
pub(crate) const BLOCK_ID_PADDING: u32 = 0x4272_6577;

/// Where a signing block starts, read from the footer before the central directory.
pub(crate) struct BlockEnvelope {
    pub(crate) start: u64,
    size: u64,
}

impl BlockEnvelope {
    /// `None` when `footer`, the bytes before the central directory at
    /// `cd_offset`, does not end with the block magic.
    pub(crate) fn from_footer(footer: &[u8], cd_offset: u64) -> Result<Option<Self>> {
        if !footer.ends_with(MAGIC) {
            return Ok(None);
        }
        let size_end = footer
            .len()
            .checked_sub(MAGIC.len())
            .ok_or_else(|| invalid("signing block", "missing magic"))?;
        let size_start = size_end
            .checked_sub(SIZE_LEN)
            .ok_or_else(|| invalid("signing block", "missing footer size"))?;
        let size = Reader::new(&footer[size_start..size_end], "signing block footer").u64()?;
        let len = size
            .checked_add(SIZE_LEN as u64)
            .filter(|len| *len >= (SIZE_LEN + FOOTER_LEN) as u64)
            .ok_or_else(|| invalid("signing block", "invalid footer size"))?;
        let start = cd_offset
            .checked_sub(len)
            .ok_or_else(|| invalid("signing block", "footer size extends before file start"))?;
        Ok(Some(Self { start, size }))
    }

    pub(crate) fn check_header(&self, header: &[u8]) -> Result<()> {
        if Reader::new(header, "signing block header").u64()? != self.size {
            return Err(invalid("signing block", "header and footer sizes differ"));
        }
        Ok(())
    }
}

/// Finds the first pair with `id` in a validated block, returning absence only
/// for a missing ID. Invalid pair lengths before the match are errors. As on
/// Android, pairs following the first match are not interpreted.
pub(crate) fn find_pair(block: &[u8], id: u32) -> Result<Option<&[u8]>> {
    for pair in pairs(block) {
        let pair = pair?;
        if pair.id == id {
            return Ok(Some(pair.value));
        }
    }
    Ok(None)
}

pub(crate) struct Pair<'a> {
    pub(crate) id: u32,
    value: &'a [u8],
    /// The whole pair, with its length and ID fields.
    pub(crate) encoded: &'a [u8],
}

/// The pairs of a validated block in order, ending after the first malformed one.
pub(crate) fn pairs(block: &[u8]) -> impl Iterator<Item = Result<Pair<'_>>> {
    let mut pairs = Reader::new(
        &block[SIZE_LEN..block.len() - FOOTER_LEN],
        "signing block pairs",
    );
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed || pairs.is_empty() {
            return None;
        }
        let pair = next_pair(&mut pairs);
        failed = pair.is_err();
        Some(pair)
    })
}

fn next_pair<'a>(pairs: &mut Reader<'a>) -> Result<Pair<'a>> {
    let remaining = pairs.data;
    let len = usize::try_from(pairs.u64()?)
        .map_err(|_| invalid("signing block", "pair length exceeds address space"))?;
    if len < 4 {
        return Err(invalid("signing block", "pair length excludes its ID"));
    }
    let mut pair = Reader::new(pairs.take(len)?, "signing pair");
    Ok(Pair {
        id: pair.u32()?,
        value: pair.take(len - 4)?,
        encoded: &remaining[..SIZE_LEN + len],
    })
}

/// The length of a block holding one pair of `value_len` bytes, `kept_len`
/// bytes of other encoded pairs, and padding.
pub(crate) const fn len_for(value_len: usize, kept_len: usize) -> usize {
    SIZE_LEN + 2 * PAIR_HEADER_LEN + value_len + kept_len + FOOTER_LEN
}

/// A block of exactly `len` bytes holding `value` under `id`, then the encoded
/// pairs `kept`, filled out with a padding pair.
pub(crate) fn write(id: u32, value: &[u8], kept: &[u8], len: usize) -> Result<Vec<u8>> {
    let padding = len
        .checked_sub(len_for(value.len(), kept.len()))
        .ok_or_else(|| invalid("signing block", "signature exceeds reserved block length"))?;
    let size = ((len - SIZE_LEN) as u64).to_le_bytes();
    let mut block = Vec::with_capacity(len);
    block.extend_from_slice(&size);
    write_pair_header(&mut block, id, value.len());
    block.extend_from_slice(value);
    block.extend_from_slice(kept);
    write_pair_header(&mut block, BLOCK_ID_PADDING, padding);
    block.resize(block.len() + padding, 0);
    block.extend_from_slice(&size);
    block.extend_from_slice(MAGIC);
    debug_assert_eq!(block.len(), len);
    Ok(block)
}

fn write_pair_header(block: &mut Vec<u8>, id: u32, value_len: usize) {
    block.extend_from_slice(&((4 + value_len) as u64).to_le_bytes());
    block.extend_from_slice(&id.to_le_bytes());
}

/// A cursor over little-endian, length-prefixed signing block fields.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
    section: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], section: &'static str) -> Self {
        Self {
            data,
            offset: 0,
            section,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let Some((value, remaining)) = self.data.split_at_checked(len) else {
            return Err(self.error("field extends past end"));
        };
        self.data = remaining;
        self.offset += len;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        bytes.copy_from_slice(self.take(N)?);
        Ok(bytes)
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    pub(crate) fn prefixed(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    fn error(&self, reason: &'static str) -> SignError {
        SignError::Malformed {
            section: self.section,
            offset: self.offset,
            reason,
        }
    }
}
