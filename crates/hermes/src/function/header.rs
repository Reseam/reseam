// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::flags::FunctionFlags;
use crate::error::{Result, invalid};
use reseam_bytes::slice;

pub(crate) const SMALL_HEADER_SIZE: usize = 12;
pub(crate) const LARGE_HEADER_SIZE: usize = 37;

/// A bit field of the small function header: `width` bits at `shift` in the
/// little-endian word at byte `word`.
#[derive(Clone, Copy)]
struct BitField {
    word: usize,
    shift: u32,
    width: u32,
}

impl BitField {
    const fn new(word: usize, shift: u32, width: u32) -> Self {
        Self { word, shift, width }
    }

    const fn fits(&self, value: u32) -> bool {
        value >> self.width == 0
    }

    fn get(&self, header: &[u8; SMALL_HEADER_SIZE]) -> u32 {
        let word = u32::from_le_bytes(
            header[self.word..self.word + 4]
                .try_into()
                .expect("small header words are in bounds"),
        );
        (word >> self.shift) & ((1 << self.width) - 1)
    }

    fn put(&self, header: &mut [u8; SMALL_HEADER_SIZE], value: u32) {
        let range = self.word..self.word + 4;
        let word = u32::from_le_bytes(
            header[range.clone()]
                .try_into()
                .expect("small header words are in bounds"),
        );
        header[range].copy_from_slice(&(word | value << self.shift).to_le_bytes());
    }
}

/// `SmallFuncHeader` fields (`FUNC_HEADER_FIELDS`), in [`FunctionHeader::fields`] order. The
/// flags occupy the last byte.
const SMALL: [BitField; FIELDS] = [
    BitField::new(0, 0, 25),
    BitField::new(0, 25, 5),
    BitField::new(0, 30, 2),
    BitField::new(4, 0, 14),
    BitField::new(4, 14, 8),
    BitField::new(4, 22, 5),
    BitField::new(4, 27, 5),
    BitField::new(8, 0, 8),
    BitField::new(8, 8, 8),
    BitField::new(8, 16, 6),
    BitField::new(8, 22, 1),
    BitField::new(8, 23, 1),
];
const FIELDS: usize = 12;
/// The large header stores the first fields as words, the rest as bytes.
const WORDS: usize = 8;
const FLAGS: usize = 11;
/// An overflowed small header splits its large header's offset between the offset and name
/// fields.
const LARGE_LOW: BitField = SMALL[0];
const LARGE_HIGH: BitField = SMALL[4];

#[derive(Debug, Clone)]
pub(crate) struct FunctionHeader {
    pub offset: u32,
    pub parameters: u32,
    pub loop_depth: u32,
    pub size: u32,
    pub name: u32,
    pub number_regs: u32,
    pub non_pointer_regs: u32,
    pub frame_size: u32,
    pub read_cache: u8,
    pub write_cache: u8,
    pub object_cache: u8,
    pub private_cache: u8,
    pub flags: FunctionFlags,
    /// Where the large header is stored, for overflowed functions. Exception
    /// handlers and debug offsets follow it.
    pub large: Option<usize>,
}

impl FunctionHeader {
    pub(crate) fn decode(source: &[u8], position: usize) -> Result<Self> {
        let small: &[u8; SMALL_HEADER_SIZE] =
            slice(source, position, SMALL_HEADER_SIZE, "function header")?
                .try_into()
                .expect("slice has the small header size");
        if small[FLAGS] & FunctionFlags::OVERFLOWED != 0 {
            return Self::decode_large(
                source,
                (LARGE_HIGH.get(small) << 24 | LARGE_LOW.get(small)) as usize,
            );
        }
        let flags = FunctionFlags::decode(small[FLAGS], position)?;
        if flags.exception_handler || flags.debug_info {
            return Err(invalid(
                position,
                "small function header with exception or debug info",
            ));
        }
        Ok(Self::new(SMALL.map(|field| field.get(small)), flags, None))
    }

    fn decode_large(source: &[u8], large: usize) -> Result<Self> {
        let data = slice(source, large, LARGE_HEADER_SIZE, "function header")?;
        let fields = std::array::from_fn(|index| match index.checked_sub(WORDS) {
            Some(byte) => u32::from(data[WORDS * 4 + byte]),
            None => u32::from_le_bytes(
                data[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("large header words are in bounds"),
            ),
        });
        let flags =
            FunctionFlags::decode(data[LARGE_HEADER_SIZE - 1], large + LARGE_HEADER_SIZE - 1)?;
        Ok(Self::new(fields, flags, Some(large)))
    }

    fn new(fields: [u32; FIELDS], flags: FunctionFlags, large: Option<usize>) -> Self {
        let [
            offset,
            parameters,
            loop_depth,
            size,
            name,
            number_regs,
            non_pointer_regs,
            frame_size,
            read_cache,
            write_cache,
            object_cache,
            private_cache,
        ] = fields;
        Self {
            offset,
            parameters,
            loop_depth,
            size,
            name,
            number_regs,
            non_pointer_regs,
            frame_size,
            read_cache: read_cache as u8,
            write_cache: write_cache as u8,
            object_cache: object_cache as u8,
            private_cache: private_cache as u8,
            flags,
            large,
        }
    }

    /// The numeric fields in header order: the words, then the cache sizes.
    fn fields(&self) -> [u32; FIELDS] {
        [
            self.offset,
            self.parameters,
            self.loop_depth,
            self.size,
            self.name,
            self.number_regs,
            self.non_pointer_regs,
            self.frame_size,
            self.read_cache.into(),
            self.write_cache.into(),
            self.object_cache.into(),
            self.private_cache.into(),
        ]
    }

    pub(crate) fn fits_small(&self) -> bool {
        SMALL
            .iter()
            .zip(self.fields())
            .all(|(field, value)| field.fits(value))
            && !self.flags.exception_handler
            && !self.flags.debug_info
    }

    /// The small header, or the overflowed form pointing at the large header at `large`.
    pub(crate) fn encode_small(&self, large: Option<u32>) -> [u8; SMALL_HEADER_SIZE] {
        let mut bytes = [0; SMALL_HEADER_SIZE];
        if let Some(large) = large {
            LARGE_LOW.put(&mut bytes, large & 0x00ff_ffff);
            LARGE_HIGH.put(&mut bytes, large >> 24);
            bytes[FLAGS] = FunctionFlags::OVERFLOWED;
        } else {
            for (field, value) in SMALL.iter().zip(self.fields()) {
                field.put(&mut bytes, value);
            }
            bytes[FLAGS] = self.flags.encode(false);
        }
        bytes
    }

    pub(crate) fn encode_large(&self) -> [u8; LARGE_HEADER_SIZE] {
        let mut bytes = [0; LARGE_HEADER_SIZE];
        for (index, value) in self.fields().into_iter().enumerate() {
            match index.checked_sub(WORDS) {
                Some(byte) => bytes[WORDS * 4 + byte] = value as u8,
                None => bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes()),
            }
        }
        bytes[LARGE_HEADER_SIZE - 1] = self.flags.encode(true);
        bytes
    }
}
