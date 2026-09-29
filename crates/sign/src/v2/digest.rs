// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;

use rayon::prelude::*;
use ring::digest::{Context, SHA256};

use crate::error::Result;
use crate::signing_block::{self, ApkSections};

pub(super) const DIGEST_LEN: usize = 32;
const CHUNK_SIZE: usize = 1 << 20;
const CHUNK_PREFIX: u8 = 0xa5;
const TOP_PREFIX: u8 = 0x5a;

pub(super) type Digest = [u8; DIGEST_LEN];

/// Chunked digest over contents, central directory, and EOCD, given the
/// contents' chunk digests. The verifier digests the EOCD with the central
/// directory offset the file would have without a signing block, so that is
/// the offset patched in here.
pub(super) fn content_digest(sections: &ApkSections<'_>, mut chunks: Vec<Digest>) -> Digest {
    let eocd = signing_block::patch_cd_offset(sections.eocd, sections.contents.len() as u32);
    chunks.extend(chunk_digests(sections.central_dir));
    chunks.extend(chunk_digests(&eocd));

    let mut ctx = Context::new(&SHA256);
    ctx.update(&[TOP_PREFIX]);
    ctx.update(&(chunks.len() as u32).to_le_bytes());
    for chunk in &chunks {
        ctx.update(chunk);
    }
    finish(ctx)
}

pub(super) fn chunk_digests(section: &[u8]) -> Vec<Digest> {
    section.par_chunks(CHUNK_SIZE).map(chunk_digest).collect()
}

/// Chunk digests of the first `len` bytes of `file`. Each chunk is mapped
/// only while it is hashed, so a large APK never becomes resident at once.
pub(super) fn file_chunk_digests(file: &File, len: u64) -> Result<Vec<Digest>> {
    let chunk_size = CHUNK_SIZE as u64;
    (0..len.div_ceil(chunk_size))
        .into_par_iter()
        .map(|index| {
            let start = index * chunk_size;
            // SAFETY: callers pass an unlinked temp file only this process holds.
            let chunk = unsafe {
                memmap2::MmapOptions::new()
                    .offset(start)
                    .len(chunk_size.min(len - start) as usize)
                    .map(file)
            }?;
            Ok(chunk_digest(&chunk))
        })
        .collect()
}

fn chunk_digest(chunk: &[u8]) -> Digest {
    let mut ctx = Context::new(&SHA256);
    ctx.update(&[CHUNK_PREFIX]);
    ctx.update(&(chunk.len() as u32).to_le_bytes());
    ctx.update(chunk);
    finish(ctx)
}

fn finish(ctx: Context) -> Digest {
    ctx.finish().as_ref().try_into().unwrap()
}
