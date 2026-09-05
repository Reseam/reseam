// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use rayon::prelude::*;
use ring::digest::{Context, SHA256};

use crate::signing_block::{self, ApkSections};

pub(super) const DIGEST_LEN: usize = 32;
const CHUNK_SIZE: usize = 1 << 20;
const CHUNK_PREFIX: u8 = 0xa5;
const TOP_PREFIX: u8 = 0x5a;

pub(super) type Digest = [u8; DIGEST_LEN];

/// Chunked digest over contents, central directory, and EOCD. The verifier
/// digests the EOCD with the central directory offset the file would have
/// without a signing block, so that is the offset patched in here.
pub(super) fn content_digest(sections: &ApkSections<'_>) -> Digest {
    let eocd = signing_block::patch_cd_offset(sections.eocd, sections.contents.len() as u32);
    let mut chunks = chunk_digests(sections.contents);
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

fn chunk_digests(section: &[u8]) -> Vec<Digest> {
    section
        .par_chunks(CHUNK_SIZE)
        .map(|chunk| {
            let mut ctx = Context::new(&SHA256);
            ctx.update(&[CHUNK_PREFIX]);
            ctx.update(&(chunk.len() as u32).to_le_bytes());
            ctx.update(chunk);
            finish(ctx)
        })
        .collect()
}

fn finish(ctx: Context) -> Digest {
    ctx.finish().as_ref().try_into().unwrap()
}
