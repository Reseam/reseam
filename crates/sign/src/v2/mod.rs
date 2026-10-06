// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod digest;
mod signer;

use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::ops::Range;

use tracing::instrument;

use crate::error::{Result, invalid, io_at};
use crate::key::SigningKey;
use crate::signing_block::{self, read_at};

/// Returns a signed copy, preserving ZIP contents and replacing any signing block.
/// Prefer `sign_file_in_place` for file-backed APKs to avoid allocating a complete copy.
#[instrument(level = "info", skip_all, fields(apk_size = apk.len()))]
pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;
    let metadata = signed_metadata(
        sections.contents.len() as u64,
        sections.eocd,
        digest::chunk_digests(sections.contents),
        digest::chunk_digests(sections.central_dir),
        key,
    )?;
    let mut output = Vec::with_capacity(
        sections.contents.len()
            + metadata.block.len()
            + sections.central_dir.len()
            + metadata.eocd.len(),
    );
    output.extend_from_slice(sections.contents);
    output.extend_from_slice(&metadata.block);
    output.extend_from_slice(sections.central_dir);
    output.extend_from_slice(&metadata.eocd);
    Ok(output)
}

/// Signs or re-signs an APK in place using bounded buffers and parallel chunk hashing.
/// Entry bytes and the central directory are preserved; the old signing block is replaced.
/// The file must be opened for reading and writing and used exclusively until completion.
/// An I/O failure during publication can leave a partially rewritten file: sign a temporary
/// output and publish it only after success. This function does not flush the file to disk.
#[instrument(level = "info", skip_all)]
pub fn sign_file_in_place(file: &File, key: &SigningKey) -> Result<()> {
    let sections = signing_block::file_sections(file)?;
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
        eocd: signer::patch_cd_offset(eocd, cd_offset),
        cd_offset,
    })
}

fn write_at(mut file: &File, offset: u64, bytes: &[u8]) -> Result<()> {
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(bytes))
        .map_err(|source| io_at("writing signed APK", offset, source))
}

fn move_range(file: &File, source: Range<u64>, destination: u64) -> Result<()> {
    if source.start == destination {
        return Ok(());
    }
    let mut buffer = vec![0; 1 << 20];
    let len = source.end - source.start;
    let mut moved = 0;
    while moved < len {
        let count = (len - moved).min(buffer.len() as u64) as usize;
        // Copy in the direction that leaves overlapping source bytes intact.
        let offset = if destination > source.start {
            len - moved - count as u64
        } else {
            moved
        };
        let chunk = &mut buffer[..count];
        read_at(file, source.start + offset, chunk)?;
        write_at(file, destination + offset, chunk)?;
        moved += count as u64;
    }
    Ok(())
}
