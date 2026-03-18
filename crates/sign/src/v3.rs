use crate::error::Result;
use crate::keystore::SigningKey;
use crate::signing_block::{self, BLOCK_ID_V2, BLOCK_ID_V3};
use crate::v2;

pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    sign_with_sdk_range(apk, key, 24, u32::MAX)
}

pub fn sign_with_sdk_range(
    apk: &[u8],
    key: &SigningKey,
    min_sdk: u32,
    max_sdk: u32,
) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;
    let target_len = target_signing_block_len(key)?;
    let _new_cd_offset = v2::checked_cd_offset(sections.contents.len(), target_len)?;
    let digest = v2::compute_content_digest(&sections)?;

    let v2_block = v2::build_v2_block_from_digest(&digest, key)?;
    let v3_block = build_v3_block_from_digest(&digest, key, min_sdk, max_sdk)?;

    let signing_block = signing_block::build_signing_block_with_padding(
        &[(BLOCK_ID_V2, v2_block), (BLOCK_ID_V3, v3_block)],
        target_len,
    )?;

    signing_block::reassemble_apk(
        sections.contents,
        &signing_block,
        sections.central_dir,
        sections.eocd,
    )
}

/// V3 wraps the v2 signer with minSDK/maxSDK fields prepended.
fn build_v3_block_from_digest(
    digest: &[u8],
    key: &SigningKey,
    min_sdk: u32,
    max_sdk: u32,
) -> Result<Vec<u8>> {
    let v2_signer = v2::build_signer_from_digest(digest, key)?;

    let mut v3_signer = Vec::new();
    v3_signer.extend_from_slice(&min_sdk.to_le_bytes());
    v3_signer.extend_from_slice(&max_sdk.to_le_bytes());
    v3_signer.extend_from_slice(&v2_signer);

    let mut signers_seq = Vec::new();
    signers_seq.extend_from_slice(&(v3_signer.len() as u32).to_le_bytes());
    signers_seq.extend_from_slice(&v3_signer);

    let mut block = Vec::new();
    block.extend_from_slice(&(signers_seq.len() as u32).to_le_bytes());
    block.extend_from_slice(&signers_seq);
    Ok(block)
}

fn target_signing_block_len(key: &SigningKey) -> Result<usize> {
    let v2_block_len = v2::max_block_len(key)?;
    let v3_block_len = v2::max_signer_len(key)? + 16; // +8 (min/max sdk) +4 (signer lp) +4 (seq lp)
    // 0 = padding pair placeholder (actual padding computed at build time)
    Ok(signing_block::signing_block_len(&[
        v2_block_len,
        v3_block_len,
        0,
    ]))
}
