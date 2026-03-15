use crate::error::{invalid, malformed, Result};
use crate::keystore::SigningKey;
use crate::signing_block::{self, ApkSections, BLOCK_ID_V2};
use ring::digest::{self, SHA256};

const SIG_ECDSA_SHA256: u32 = 0x0201;
const CHUNK_SIZE: usize = 1 << 20; // 1 MB
const DIGEST_LEN: usize = 32;
const MAX_ECDSA_DER_SIGNATURE_LEN: usize = 72;

pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;
    let target_len = target_signing_block_len(key)?;
    let new_cd_offset = checked_cd_offset(sections.contents.len(), target_len)?;
    let v2_block = build_v2_block_from_sections(&sections, key, new_cd_offset)?;
    let signing_block =
        signing_block::build_signing_block_with_padding(&[(BLOCK_ID_V2, v2_block)], target_len)?;
    signing_block::reassemble_apk(
        sections.contents,
        &signing_block,
        sections.central_dir,
        sections.eocd,
    )
}

pub(crate) fn build_v2_block_from_sections(
    sections: &ApkSections<'_>,
    key: &SigningKey,
    new_cd_offset: u32,
) -> Result<Vec<u8>> {
    let digest = compute_content_digest(sections, new_cd_offset)?;
    build_v2_block_from_digest(&digest, key)
}

pub(crate) fn build_v2_block_from_digest(digest: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let signed_data = build_signed_data(digest, key.certificate_der())?;
    let signature = key.sign(&signed_data)?;
    let signer = build_signer(&signed_data, &signature, key)?;

    let mut block = Vec::new();
    write_lp(&mut block, &signer);
    Ok(block)
}

pub(crate) fn build_signer_from_digest(digest: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let signed_data = build_signed_data(digest, key.certificate_der())?;
    let signature = key.sign(&signed_data)?;
    build_signer(&signed_data, &signature, key)
}

/// Chunked Merkle digest: each 1MB chunk hashed with 0xa5 prefix,
/// combined with 0x5a prefix. Sections: contents, central dir, EOCD.
pub(crate) fn compute_content_digest(
    sections: &ApkSections<'_>,
    new_cd_offset: u32,
) -> Result<Vec<u8>> {
    // EOCD must reflect the post-signing CD offset for correct digest
    let mut patched_eocd = sections.eocd.to_vec();
    if patched_eocd.len() >= 22 {
        patched_eocd[16..20].copy_from_slice(&new_cd_offset.to_le_bytes());
    }

    let mut chunk_digests = Vec::new();
    digest_section_chunks(sections.contents, &mut chunk_digests);
    digest_section_chunks(sections.central_dir, &mut chunk_digests);
    digest_section_chunks(&patched_eocd, &mut chunk_digests);

    let mut top_input = vec![0x5a];
    top_input.extend_from_slice(&(chunk_digests.len() as u32).to_le_bytes());
    for cd in &chunk_digests {
        top_input.extend_from_slice(cd);
    }

    Ok(digest::digest(&SHA256, &top_input).as_ref().to_vec())
}

pub(crate) fn checked_cd_offset(contents_len: usize, signing_block_len: usize) -> Result<u32> {
    let total_len = contents_len.checked_add(signing_block_len).ok_or_else(|| {
        invalid(
            "apk",
            "APK size overflowed while computing central directory offset",
        )
    })?;

    u32::try_from(total_len)
        .map_err(|_| invalid("apk", "APK exceeds ZIP32 central directory offset limits"))
}

pub(crate) fn max_signer_len(key: &SigningKey) -> Result<usize> {
    let digest = [0u8; DIGEST_LEN];
    let signed_data = build_signed_data(&digest, key.certificate_der())?;
    let spki_len = crate::der::ec_subject_public_key_info(key.public_key_bytes()).len();

    signed_data
        .len()
        .checked_add(spki_len)
        .and_then(|len| len.checked_add(MAX_ECDSA_DER_SIGNATURE_LEN + 24))
        .ok_or_else(|| {
            malformed(
                "signing block",
                signed_data.len(),
                "v2 signer length overflowed while sizing signing block",
            )
        })
}

pub(crate) fn max_block_len(key: &SigningKey) -> Result<usize> {
    max_signer_len(key)?.checked_add(4).ok_or_else(|| {
        malformed(
            "signing block",
            0,
            "v2 block length overflowed while sizing signing block",
        )
    })
}

fn target_signing_block_len(key: &SigningKey) -> Result<usize> {
    Ok(signing_block::signing_block_len(&[max_block_len(key)?, 0]))
}

fn digest_section_chunks(data: &[u8], chunk_digests: &mut Vec<Vec<u8>>) {
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + CHUNK_SIZE).min(data.len());
        let chunk = &data[offset..end];

        let mut ctx = digest::Context::new(&SHA256);
        ctx.update(&[0xa5]);
        ctx.update(&(chunk.len() as u32).to_le_bytes());
        ctx.update(chunk);
        chunk_digests.push(ctx.finish().as_ref().to_vec());

        offset = end;
    }
}

fn build_signed_data(digest: &[u8], certificate_der: &[u8]) -> Result<Vec<u8>> {
    let mut signed_data = Vec::new();

    // Digests
    let mut digests = Vec::new();
    let mut entry = Vec::new();
    entry.extend_from_slice(&SIG_ECDSA_SHA256.to_le_bytes());
    write_lp(&mut entry, digest);
    write_lp(&mut digests, &entry);
    write_lp(&mut signed_data, &digests);

    // Certificates
    let mut certs = Vec::new();
    write_lp(&mut certs, certificate_der);
    write_lp(&mut signed_data, &certs);

    // Additional attributes (empty)
    write_lp(&mut signed_data, &[]);

    Ok(signed_data)
}

fn build_signer(signed_data: &[u8], signature: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let mut signer = Vec::new();

    write_lp(&mut signer, signed_data);

    let mut sigs = Vec::new();
    let mut entry = Vec::new();
    entry.extend_from_slice(&SIG_ECDSA_SHA256.to_le_bytes());
    write_lp(&mut entry, signature);
    write_lp(&mut sigs, &entry);
    write_lp(&mut signer, &sigs);

    let spki = crate::der::ec_subject_public_key_info(key.public_key_bytes());
    write_lp(&mut signer, &spki);

    Ok(signer)
}

fn write_lp(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}
