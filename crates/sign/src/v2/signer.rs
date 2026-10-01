// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::digest::{DIGEST_LEN, Digest};
use crate::error::{Result, invalid};
use crate::key::SigningKey;
use crate::signing_block::{
    APK_SIG_BLOCK_MAGIC, BLOCK_ID_V2, BLOCK_OVERHEAD, EOCD_CD_OFFSET_FIELD,
};

const SIG_ECDSA_SHA256: u32 = 0x0201;
const MAX_ECDSA_DER_SIGNATURE_LEN: usize = 72;
const LENGTH_PREFIX: usize = 4;

fn block(digest: &Digest, key: &SigningKey) -> Result<Vec<u8>> {
    let signed_data = signed_data(digest, key.certificate_der());
    let signature = key.sign(&signed_data)?;
    let spki = key.public_key_der();
    let signer = signer(&signed_data, &signature, spki);
    Ok(length_prefixed(&length_prefixed(&signer)))
}

pub(super) fn signing_block_len(key: &SigningKey) -> usize {
    let signed_data = signed_data(&[0; DIGEST_LEN], key.certificate_der());
    let spki = key.public_key_der();
    // DER ECDSA signatures vary in length, so reserve the longest signature and padding.
    BLOCK_OVERHEAD
        + 2 * PAIR_OVERHEAD
        + signer(&signed_data, &[0; MAX_ECDSA_DER_SIGNATURE_LEN], spki).len()
        + 2 * LENGTH_PREFIX
}

fn signed_data(digest: &[u8], certificate_der: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_lp(&mut out, &length_prefixed(&algorithm_entry(digest)));
    write_lp(&mut out, &length_prefixed(certificate_der));
    write_lp(&mut out, &[]);
    out
}

fn signer(signed_data: &[u8], signature: &[u8], spki: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_lp(&mut out, signed_data);
    write_lp(&mut out, &length_prefixed(&algorithm_entry(signature)));
    write_lp(&mut out, spki);
    out
}

fn algorithm_entry(payload: &[u8]) -> Vec<u8> {
    let mut out = SIG_ECDSA_SHA256.to_le_bytes().to_vec();
    write_lp(&mut out, payload);
    out
}

fn length_prefixed(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(LENGTH_PREFIX + data.len());
    write_lp(&mut out, data);
    out
}

fn write_lp(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

const PAIR_OVERHEAD: usize = 12;
const BLOCK_ID_PADDING: u32 = 0x4272_6577;

pub(super) fn signing_block(
    digest: &Digest,
    key: &SigningKey,
    target_len: usize,
) -> Result<Vec<u8>> {
    let value = block(digest, key)?;
    let padding = target_len
        .checked_sub(BLOCK_OVERHEAD + 2 * PAIR_OVERHEAD + value.len())
        .ok_or_else(|| invalid("signing block", "signature exceeds reserved block length"))?;
    let size = (target_len - 8) as u64;
    let mut block = Vec::with_capacity(target_len);
    block.extend_from_slice(&size.to_le_bytes());
    write_pair_header(&mut block, BLOCK_ID_V2, value.len());
    block.extend_from_slice(&value);
    write_pair_header(&mut block, BLOCK_ID_PADDING, padding);
    block.resize(block.len() + padding, 0);
    block.extend_from_slice(&size.to_le_bytes());
    block.extend_from_slice(APK_SIG_BLOCK_MAGIC);
    debug_assert_eq!(block.len(), target_len);
    Ok(block)
}

pub(super) fn patch_cd_offset(eocd: &[u8], cd_offset: u32) -> Vec<u8> {
    let mut patched = eocd.to_vec();
    patched[EOCD_CD_OFFSET_FIELD].copy_from_slice(&cd_offset.to_le_bytes());
    patched
}

fn write_pair_header(block: &mut Vec<u8>, id: u32, value_len: usize) {
    block.extend_from_slice(&((4 + value_len) as u64).to_le_bytes());
    block.extend_from_slice(&id.to_le_bytes());
}
