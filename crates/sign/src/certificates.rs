// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::signing_block::{self, BLOCK_ID_V2, BLOCK_ID_V3, Reader};

/// Reads each signer's leaf certificate, choosing v3 when present, otherwise v2.
/// Unsigned and JAR-only signed archives return an empty list. Malformed block
/// envelopes, pair framing and certificate framing are errors; signatures and
/// certificate trust are not verified by this extraction operation.
pub fn signer_certificates(apk: &[u8]) -> Result<Vec<Vec<u8>>> {
    let Some(block) = signing_block::block(apk)? else {
        return Ok(Vec::new());
    };
    let v3 = signing_block::find_pair(block, BLOCK_ID_V3);
    let signers = match v3 {
        Ok(Some(signers)) => signers,
        Ok(None) => match signing_block::find_pair(block, BLOCK_ID_V2)? {
            Some(signers) => signers,
            None => return Ok(Vec::new()),
        },
        Err(error) => {
            // Android can use v2 when a later malformed pair prevents locating v3.
            let Some(signers) = signing_block::find_pair(block, BLOCK_ID_V2)? else {
                return Err(error);
            };
            signers
        }
    };
    let signers = Reader::new(signers, "signers").prefixed()?;
    if signers.is_empty() {
        return Err(invalid("signers", "no signers in scheme block"));
    }
    let mut signers = Reader::new(signers, "signers");
    let mut certificates = Vec::new();
    while !signers.is_empty() {
        let signer = signers.prefixed()?;
        let signed_data = Reader::new(signer, "signer").prefixed()?;
        let mut signed_data = Reader::new(signed_data, "signed data");
        signed_data.prefixed()?;
        let mut chain = Reader::new(signed_data.prefixed()?, "certificates");
        let leaf = chain.prefixed()?;
        if leaf.is_empty() {
            return Err(invalid("certificates", "empty leaf certificate"));
        }
        certificates.push(leaf.to_vec());
        while !chain.is_empty() {
            chain.prefixed()?;
        }
    }
    Ok(certificates)
}
