// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The v2 block: a length-prefixed sequence holding one signer.

use super::digest::{Digest, DIGEST_LEN};
use crate::der;
use crate::error::Result;
use crate::key::SigningKey;

const SIG_ECDSA_SHA256: u32 = 0x0201;
const MAX_ECDSA_DER_SIGNATURE_LEN: usize = 72;
const LENGTH_PREFIX: usize = 4;

pub(super) fn block(digest: &Digest, key: &SigningKey) -> Result<Vec<u8>> {
    let signed_data = signed_data(digest, key.certificate_der());
    let signature = key.sign(&signed_data)?;
    let spki = der::ec_subject_public_key_info(key.public_key_bytes());
    let signer = signer(&signed_data, &signature, &spki);
    Ok(length_prefixed(&length_prefixed(&signer)))
}

/// Upper bound on [`block`] for this key. ECDSA DER signatures vary in length
/// per signing, so the block is sized for the longest one.
pub(super) fn max_block_len(key: &SigningKey) -> usize {
    let signed_data = signed_data(&[0; DIGEST_LEN], key.certificate_der());
    let spki = der::ec_subject_public_key_info(key.public_key_bytes());
    signer(&signed_data, &[0; MAX_ECDSA_DER_SIGNATURE_LEN], &spki).len() + 2 * LENGTH_PREFIX
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
