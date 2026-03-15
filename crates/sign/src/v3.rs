use crate::error::Result;
use crate::keystore::SigningKey;
use crate::signing_block::{self, ApkSections, BLOCK_ID_V2, BLOCK_ID_V3};
use crate::v2;

/// Sign an APK with APK Signature Scheme v3.
///
/// V3 extends v2 with SDK version targeting. For patched APKs we target
/// minSDK=24 (Android 7.0, where v2 was introduced) through maxSDK=u32::MAX.
pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    sign_with_sdk_range(apk, key, 24, u32::MAX)
}

/// Sign with explicit SDK version range.
pub fn sign_with_sdk_range(
    apk: &[u8],
    key: &SigningKey,
    min_sdk: u32,
    max_sdk: u32,
) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;

    let v2_block = build_v2_block_value(&sections, key)?;
    let v3_block = build_v3_block_value(&sections, key, min_sdk, max_sdk)?;

    let signing_block = signing_block::build_signing_block(&[
        (BLOCK_ID_V2, v2_block),
        (BLOCK_ID_V3, v3_block),
    ]);

    signing_block::reassemble_apk(
        sections.contents,
        &signing_block,
        sections.central_dir,
        sections.eocd,
    )
}

/// Build the v2 block value (reuses v2 logic).
fn build_v2_block_value(sections: &ApkSections<'_>, key: &SigningKey) -> Result<Vec<u8>> {
    // Use v2's internal block builder
    v2::build_v2_block_from_sections(sections, key)
}

/// Build the v3 block value.
///
/// V3 signer format is the same as v2 but with minSDK/maxSDK prepended to each signer.
fn build_v3_block_value(
    sections: &ApkSections<'_>,
    key: &SigningKey,
    min_sdk: u32,
    max_sdk: u32,
) -> Result<Vec<u8>> {
    let v2_signer = v2::build_signer_from_sections(sections, key)?;

    // V3 signer wraps v2 signer with SDK range
    let mut v3_signer = Vec::new();
    v3_signer.extend_from_slice(&min_sdk.to_le_bytes());
    v3_signer.extend_from_slice(&max_sdk.to_le_bytes());
    v3_signer.extend_from_slice(&v2_signer);

    // Length-prefixed sequence of signers
    let mut block = Vec::new();
    let signer_len = v3_signer.len() as u32;
    block.extend_from_slice(&signer_len.to_le_bytes());
    block.extend_from_slice(&v3_signer);

    Ok(block)
}
