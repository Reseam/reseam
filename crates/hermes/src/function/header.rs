// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::flags::FunctionFlags;
use crate::bytes::{read_u32, slice};
use crate::error::{Result, invalid};

pub(crate) const SMALL_HEADER_SIZE: usize = 12;
pub(crate) const LARGE_HEADER_SIZE: usize = 37;

/// A bit field of the small function header: `width` bits at `shift` in the
/// little-endian word at byte `word`.
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

/// `SmallFuncHeader` fields (`FUNC_HEADER_FIELDS`). The flags occupy the last byte.
const OFFSET: BitField = BitField::new(0, 0, 25);
const PARAMETERS: BitField = BitField::new(0, 25, 5);
const LOOP_DEPTH: BitField = BitField::new(0, 30, 2);
const SIZE: BitField = BitField::new(4, 0, 14);
const NAME: BitField = BitField::new(4, 14, 8);
const NUMBER_REGS: BitField = BitField::new(4, 22, 5);
const NON_POINTER_REGS: BitField = BitField::new(4, 27, 5);
const FRAME_SIZE: BitField = BitField::new(8, 0, 8);
const READ_CACHE: BitField = BitField::new(8, 8, 8);
const WRITE_CACHE: BitField = BitField::new(8, 16, 6);
const OBJECT_CACHE: BitField = BitField::new(8, 22, 1);
const PRIVATE_CACHE: BitField = BitField::new(8, 23, 1);
const FLAGS: usize = 11;
/// An overflowed small header splits its large header's offset between these fields.
const LARGE_LOW: BitField = OFFSET;
const LARGE_HIGH: BitField = NAME;

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
        let small: &[u8; SMALL_HEADER_SIZE] = slice(source, position, SMALL_HEADER_SIZE)?
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
        Ok(Self {
            offset: OFFSET.get(small),
            parameters: PARAMETERS.get(small),
            loop_depth: LOOP_DEPTH.get(small),
            size: SIZE.get(small),
            name: NAME.get(small),
            number_regs: NUMBER_REGS.get(small),
            non_pointer_regs: NON_POINTER_REGS.get(small),
            frame_size: FRAME_SIZE.get(small),
            read_cache: READ_CACHE.get(small) as u8,
            write_cache: WRITE_CACHE.get(small) as u8,
            object_cache: OBJECT_CACHE.get(small) as u8,
            private_cache: PRIVATE_CACHE.get(small) as u8,
            flags,
            large: None,
        })
    }

    fn decode_large(source: &[u8], large: usize) -> Result<Self> {
        let data = slice(source, large, LARGE_HEADER_SIZE)?;
        let word = |index: usize| read_u32(data, index * 4);
        Ok(Self {
            offset: word(0)?,
            parameters: word(1)?,
            loop_depth: word(2)?,
            size: word(3)?,
            name: word(4)?,
            number_regs: word(5)?,
            non_pointer_regs: word(6)?,
            frame_size: word(7)?,
            read_cache: data[32],
            write_cache: data[33],
            object_cache: data[34],
            private_cache: data[35],
            flags: FunctionFlags::decode(data[36], large + 36)?,
            large: Some(large),
        })
    }

    pub(crate) fn fits_small(&self) -> bool {
        OFFSET.fits(self.offset)
            && PARAMETERS.fits(self.parameters)
            && LOOP_DEPTH.fits(self.loop_depth)
            && SIZE.fits(self.size)
            && NAME.fits(self.name)
            && NUMBER_REGS.fits(self.number_regs)
            && NON_POINTER_REGS.fits(self.non_pointer_regs)
            && FRAME_SIZE.fits(self.frame_size)
            && WRITE_CACHE.fits(self.write_cache.into())
            && OBJECT_CACHE.fits(self.object_cache.into())
            && PRIVATE_CACHE.fits(self.private_cache.into())
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
            OFFSET.put(&mut bytes, self.offset);
            PARAMETERS.put(&mut bytes, self.parameters);
            LOOP_DEPTH.put(&mut bytes, self.loop_depth);
            SIZE.put(&mut bytes, self.size);
            NAME.put(&mut bytes, self.name);
            NUMBER_REGS.put(&mut bytes, self.number_regs);
            NON_POINTER_REGS.put(&mut bytes, self.non_pointer_regs);
            FRAME_SIZE.put(&mut bytes, self.frame_size);
            READ_CACHE.put(&mut bytes, self.read_cache.into());
            WRITE_CACHE.put(&mut bytes, self.write_cache.into());
            OBJECT_CACHE.put(&mut bytes, self.object_cache.into());
            PRIVATE_CACHE.put(&mut bytes, self.private_cache.into());
            bytes[FLAGS] = self.flags.encode(false);
        }
        bytes
    }

    pub(crate) fn encode_large(&self) -> [u8; LARGE_HEADER_SIZE] {
        let mut bytes = [0; LARGE_HEADER_SIZE];
        for (index, value) in [
            self.offset,
            self.parameters,
            self.loop_depth,
            self.size,
            self.name,
            self.number_regs,
            self.non_pointer_regs,
            self.frame_size,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[32..].copy_from_slice(&[
            self.read_cache,
            self.write_cache,
            self.object_cache,
            self.private_cache,
            self.flags.encode(true),
        ]);
        bytes
    }
}
