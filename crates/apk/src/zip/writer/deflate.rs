// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Write;

use flate2::{Compress, Compression, Crc, FlushCompress, Status};
use rayon::prelude::*;

use super::Source;
use crate::error::{Result, invalid};

/// Input one task compresses.
const BLOCK: u64 = 128 << 10;
/// History a block's compressor is primed with: the whole deflate window, so
/// the stream compresses about as well as one built in a single pass.
const WINDOW: u64 = 32 << 10;

/// A source split into blocks that compress independently and join into one
/// raw deflate stream: each block is primed with the data before it and ends
/// on a byte boundary, the last one closing the stream.
#[derive(Debug, Clone, Copy)]
struct Blocks {
    len: u64,
    count: u64,
}

/// One compressed block and the CRC-32 of its input.
struct DeflatedBlock {
    data: Vec<u8>,
    crc: Crc,
}

impl Blocks {
    fn of(len: u64) -> Self {
        Self {
            len,
            count: len.div_ceil(BLOCK).max(1),
        }
    }

    fn count(self) -> u64 {
        self.count
    }

    fn deflate(
        self,
        source: &(impl Source + ?Sized),
        block: u64,
        compress: &mut Compress,
    ) -> Result<DeflatedBlock> {
        let start = block * BLOCK;
        let history = start.min(WINDOW);
        let end = (start + BLOCK).min(self.len);
        let mut input = vec![0; (end - start + history) as usize];
        source.read_into(&mut input, start - history)?;
        let (history, data) = input.split_at(history as usize);
        let mut crc = Crc::new();
        crc.update(data);
        compress.reset();
        if !history.is_empty() {
            compress
                .set_dictionary(history)
                .map_err(|error| invalid("deflate", error.to_string()))?;
        }
        let last = block + 1 == self.count;
        let flush = if last {
            FlushCompress::Finish
        } else {
            FlushCompress::Sync
        };
        let mut out = Vec::with_capacity(data.len() / 2 + 64);
        loop {
            let consumed = compress.total_in() as usize;
            let status = compress
                .compress_vec(&data[consumed..], &mut out, flush)
                .map_err(|error| invalid("deflate", error.to_string()))?;
            let flushed =
                !last && compress.total_in() as usize == data.len() && out.len() < out.capacity();
            if status == Status::StreamEnd || flushed {
                return Ok(DeflatedBlock { data: out, crc });
            }
            out.reserve(out.capacity().max(64));
        }
    }
}

fn compressor(level: u32) -> Compress {
    Compress::new(Compression::new(level), false)
}

/// Deflates `len` bytes of `source` into `out` and returns their CRC-32,
/// compressing a group of blocks, one per thread, at a time.
pub(super) fn deflate(
    source: &(impl Source + ?Sized),
    len: u64,
    level: u32,
    out: &mut impl Write,
) -> Result<Crc> {
    let blocks = Blocks::of(len);
    let group = rayon::current_num_threads() as u64;
    let mut crc = Crc::new();
    for first in (0..blocks.count()).step_by(group as usize) {
        let compressed: Vec<DeflatedBlock> = (first..(first + group).min(blocks.count()))
            .into_par_iter()
            .map_init(
                || compressor(level),
                |compress, block| blocks.deflate(source, block, compress),
            )
            .collect::<Result<_>>()?;
        for block in compressed {
            out.write_all(&block.data)?;
            crc.combine(&block.crc);
        }
    }
    Ok(crc)
}
