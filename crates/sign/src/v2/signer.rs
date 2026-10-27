// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::digest::{DIGEST_LEN, Digest};
use crate::block::{self, BLOCK_ID_V2};
use crate::error::Result;
use crate::key::SigningKey;

const SIG_ECDSA_SHA256: u32 = 0x0201;
const MAX_ECDSA_DER_SIGNATURE_LEN: usize = 72;
const LENGTH_PREFIX: usize = 4;

/// The signing block holding `key`'s v2 signature over `digest`, `len` bytes long.
pub(super) fn signing_block(digest: &Digest, key: &SigningKey, len: usize) -> Result<Vec<u8>> {
    let signed_data = signed_data(digest, key.certificate_der());
    let signature = key.sign(&signed_data)?;
    let signers = signers(&signer(&signed_data, &signature, key.public_key_der()));
    block::write(BLOCK_ID_V2, &signers, len)
}

pub(super) fn signing_block_len(key: &SigningKey) -> usize {
    let signed_data = signed_data(&[0; DIGEST_LEN], key.certificate_der());
    let spki = key.public_key_der();
    // DER ECDSA signatures vary in length, so reserve the longest signature and padding.
    let signer = signer(&signed_data, &[0; MAX_ECDSA_DER_SIGNATURE_LEN], spki);
    block::len_for(signers(&signer).len())
}

/// The v2 scheme value: a sequence of one signer.
fn signers(signer: &[u8]) -> Vec<u8> {
    length_prefixed(&length_prefixed(signer))
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
