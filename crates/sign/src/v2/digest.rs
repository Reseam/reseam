// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::ops::Range;

use rayon::prelude::*;
use ring::digest::{Context, SHA256};

use super::signer;
use crate::error::{Result, invalid, io_at};

pub(super) const DIGEST_LEN: usize = 32;
const CHUNK_SIZE: usize = 1 << 20;
const CHUNK_PREFIX: u8 = 0xa5;
const TOP_PREFIX: u8 = 0x5a;

pub(super) type Digest = [u8; DIGEST_LEN];

pub(super) fn content_digest(
    mut contents: Vec<Digest>,
    central_dir: Vec<Digest>,
    eocd: &[u8],
    contents_len: u32,
) -> Result<Digest> {
    // Android hashes the EOCD with the offset it would have without the signing block.
    let eocd = signer::patch_cd_offset(eocd, contents_len);
    contents.extend(central_dir);
    contents.extend(chunk_digests(&eocd));
    let count = u32::try_from(contents.len())
        .map_err(|_| invalid("APK digest", "chunk count exceeds 32-bit length"))?;
    let mut ctx = Context::new(&SHA256);
    ctx.update(&[TOP_PREFIX]);
    ctx.update(&count.to_le_bytes());
    for chunk in &contents {
        ctx.update(chunk);
    }
    Ok(finish(ctx))
}

pub(super) fn chunk_digests(section: &[u8]) -> Vec<Digest> {
    section.par_chunks(CHUNK_SIZE).map(chunk_digest).collect()
}

pub(super) fn file_chunk_digests(file: &File, section: Range<u64>) -> Result<Vec<Digest>> {
    let len = section.end - section.start;
    let count = usize::try_from(len.div_ceil(CHUNK_SIZE as u64))
        .map_err(|_| invalid("APK digest", "chunk count exceeds address space"))?;
    let mut digests = vec![[0; DIGEST_LEN]; count];
    if count == 0 {
        return Ok(digests);
    }
    // Limit resident chunk mappings even when the host has a large Rayon pool.
    let group_len = count.div_ceil(rayon::current_num_threads().min(8));
    digests.par_chunks_mut(group_len).enumerate().try_for_each(
        |(group, digests)| -> Result<()> {
            for (index, digest) in digests.iter_mut().enumerate() {
                let relative = (group * group_len + index) as u64 * CHUNK_SIZE as u64;
                let count = (len - relative).min(CHUNK_SIZE as u64) as usize;
                // SAFETY: signing requires exclusive use of the file until completion;
                // the validated range is hashed before any in-place writes occur.
                let chunk =
                    unsafe { reseam_storage::map_range(file, section.start + relative, count) }
                        .map_err(|source| {
                            io_at("mapping APK digest chunk", section.start + relative, source)
                        })?;
                *digest = chunk_digest(&chunk);
            }
            Ok(())
        },
    )?;
    Ok(digests)
}

fn chunk_digest(chunk: &[u8]) -> Digest {
    let mut ctx = Context::new(&SHA256);
    ctx.update(&[CHUNK_PREFIX]);
    ctx.update(&(chunk.len() as u32).to_le_bytes());
    ctx.update(chunk);
    finish(ctx)
}

fn finish(ctx: Context) -> Digest {
    ctx.finish()
        .as_ref()
        .try_into()
        .expect("SHA-256 produces 32 bytes")
}
