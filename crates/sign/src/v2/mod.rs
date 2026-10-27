// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod digest;
mod signer;

use std::fs::File;

use tracing::instrument;

use crate::error::{Result, invalid, io_at};
use crate::key::SigningKey;
use crate::zip_layout::{self, move_range, patch_cd_offset, write_at};

/// Signs or re-signs an APK in place using bounded buffers and parallel chunk hashing.
/// Entry bytes and the central directory are preserved; the old signing block is replaced.
/// The file must be opened for reading and writing and used exclusively until completion.
/// An I/O failure during publication can leave a partially rewritten file: sign a temporary
/// output and publish it only after success. This function does not flush the file to disk.
#[instrument(level = "info", skip_all)]
pub fn sign_file_in_place(file: &File, key: &SigningKey) -> Result<()> {
    let sections = zip_layout::sections(file)?;
    let metadata = signed_metadata(
        sections.contents_len,
        &sections.eocd,
        digest::file_chunk_digests(file, 0..sections.contents_len)?,
        digest::file_chunk_digests(file, sections.central_dir.clone())?,
        key,
    )?;
    let cd_len = sections.central_dir.end - sections.central_dir.start;
    move_range(file, sections.central_dir, u64::from(metadata.cd_offset))?;
    write_at(file, sections.contents_len, &metadata.block)?;
    let eocd_offset = u64::from(metadata.cd_offset) + cd_len;
    write_at(file, eocd_offset, &metadata.eocd)?;
    let end = eocd_offset + metadata.eocd.len() as u64;
    file.set_len(end)
        .map_err(|source| io_at("truncating signed APK", end, source))?;
    Ok(())
}

struct SignedMetadata {
    block: Vec<u8>,
    eocd: Vec<u8>,
    cd_offset: u32,
}

fn signed_metadata(
    contents_len: u64,
    eocd: &[u8],
    contents: Vec<digest::Digest>,
    central_dir: Vec<digest::Digest>,
    key: &SigningKey,
) -> Result<SignedMetadata> {
    let block_len = signer::signing_block_len(key);
    let cd_offset = u32::try_from(contents_len + block_len as u64)
        .map_err(|_| invalid("apk", "central directory offset exceeds ZIP32 limits"))?;
    let digest = digest::content_digest(contents, central_dir, eocd, contents_len as u32)?;
    let block = signer::signing_block(&digest, key, block_len)?;
    Ok(SignedMetadata {
        block,
        eocd: patch_cd_offset(eocd, cd_offset),
        cd_offset,
    })
}
