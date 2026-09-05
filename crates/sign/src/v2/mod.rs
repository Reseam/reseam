// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! APK Signature Scheme v2.

mod digest;
mod signer;

use std::fs::File;
use std::os::unix::fs::FileExt;

use tracing::instrument;

use crate::error::{invalid, Result};
use crate::key::SigningKey;
use crate::signing_block::{self, ApkSections, BLOCK_ID_V2};

#[instrument(level = "info", skip_all, fields(apk_size = apk.len()))]
pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;
    let tail = signed_tail(&sections, key)?;
    let mut output = Vec::with_capacity(sections.contents.len() + tail.len());
    output.extend_from_slice(sections.contents);
    output.extend_from_slice(&tail);
    Ok(output)
}

/// Signs an unsigned APK where it is: the contents stay untouched and only the
/// bytes after them are rewritten, so a large APK is never copied.
#[instrument(level = "info", skip_all)]
pub fn sign_file_in_place(file: &File, key: &SigningKey) -> Result<()> {
    // SAFETY: callers pass an unlinked temp file only this process holds, so
    // the mapping cannot change underneath us.
    let mapped = unsafe { memmap2::Mmap::map(file) }?;
    let sections = signing_block::split_apk(&mapped)?;
    let contents_len = sections.contents.len() as u64;
    let tail = signed_tail(&sections, key)?;
    drop(mapped);
    file.write_all_at(&tail, contents_len)?;
    file.set_len(contents_len + tail.len() as u64)?;
    Ok(())
}

/// Signing block, central directory, and an EOCD pointing past the block.
fn signed_tail(sections: &ApkSections<'_>, key: &SigningKey) -> Result<Vec<u8>> {
    let block_len = signing_block::signing_block_len([signer::max_block_len(key), 0]);
    let cd_offset = u32::try_from(sections.contents.len() + block_len)
        .map_err(|_| invalid("apk", "central directory offset exceeds ZIP32 limits"))?;
    let v2_block = signer::block(&digest::content_digest(sections), key)?;
    let signing_block = signing_block::build_signing_block(&[(BLOCK_ID_V2, v2_block)], block_len)?;
    let eocd = signing_block::patch_cd_offset(sections.eocd, cd_offset);
    let mut tail =
        Vec::with_capacity(signing_block.len() + sections.central_dir.len() + eocd.len());
    tail.extend_from_slice(&signing_block);
    tail.extend_from_slice(sections.central_dir);
    tail.extend_from_slice(&eocd);
    Ok(tail)
}
