// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading signer certificates back out of a signed APK.

use crate::error::{invalid, Result};
use crate::signing_block::{self, BLOCK_ID_V2, BLOCK_ID_V3};

/// The DER-encoded X.509 certificate of every signer, read from the APK
/// Signing Block: the v3 signers when present, otherwise v2. Empty when the
/// archive is unsigned or carries only a JAR (v1) signature. Each certificate
/// matches what `PackageManager` reports as a signer's `Signature`.
pub fn signer_certificates(apk: &[u8]) -> Result<Vec<Vec<u8>>> {
    let Some(block) = signing_block::block(apk)? else {
        return Ok(Vec::new());
    };
    let Some(signers) = signing_block::find_pair(block, BLOCK_ID_V3)
        .or_else(|| signing_block::find_pair(block, BLOCK_ID_V2))
    else {
        return Ok(Vec::new());
    };
    leaf_certificates(signers)
}

/// Walks the length-prefixed signers sequence, collecting each signer's leaf
/// certificate. The block value, its signers, and their signed data are all
/// length-prefixed, and certificates are the second field of the signed data
/// in both v2 and v3, so one walk serves both schemes.
fn leaf_certificates(signers_block: &[u8]) -> Result<Vec<Vec<u8>>> {
    let signers = lp(signers_block, 0)?;
    let mut certificates = Vec::new();
    let mut pos = 0;
    while pos < signers.len() {
        let signer = lp(signers, pos)?;
        pos += 4 + signer.len();
        let signed_data = lp(signer, 0)?;
        let digests = lp(signed_data, 0)?;
        let certs = lp(signed_data, 4 + digests.len())?;
        certificates.push(lp(certs, 0)?.to_vec());
    }
    Ok(certificates)
}

/// The length-prefixed slice at `offset`: a little-endian `u32` length and the
/// bytes that follow it. An out-of-range range yields `None` from `get`, so a
/// malformed or overflowing length is reported, never trusted.
fn lp(data: &[u8], offset: usize) -> Result<&[u8]> {
    let len = data
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("signing block", "truncated length prefix"))?;
    let len = u32::from_le_bytes(len.try_into().unwrap()) as usize;
    data.get(offset + 4..offset + 4 + len)
        .ok_or_else(|| invalid("signing block", "length prefix past end"))
}
