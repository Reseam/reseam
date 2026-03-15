use crate::error::Result;
use crate::keystore::SigningKey;
use crate::signing_block::{self, ApkSections, BLOCK_ID_V2};
use ring::digest::{self, SHA256};

/// APK Signature Scheme v2 algorithm ID: ECDSA with SHA-256.
const SIG_ECDSA_SHA256: u32 = 0x0201;

// Content digest uses chunked SHA-256

/// Chunk size for content digesting: 1 MB.
const CHUNK_SIZE: usize = 1 << 20;

/// Sign an APK with APK Signature Scheme v2.
///
/// Takes the raw APK bytes and returns signed APK bytes with the signing block inserted.
pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let sections = signing_block::split_apk(apk)?;
    let v2_block = build_v2_block(&sections, key)?;

    let signing_block =
        signing_block::build_signing_block(&[(BLOCK_ID_V2, v2_block)]);

    // Reassemble with the signing block's position accounted for in EOCD
    signing_block::reassemble_apk(
        sections.contents,
        &signing_block,
        sections.central_dir,
        sections.eocd,
    )
}

/// Build the v2 signature block value from pre-split sections.
pub(crate) fn build_v2_block_from_sections(sections: &ApkSections<'_>, key: &SigningKey) -> Result<Vec<u8>> {
    build_v2_block(sections, key)
}

/// Build a raw signer block (without the outer length-prefix wrapper) from sections.
pub(crate) fn build_signer_from_sections(sections: &ApkSections<'_>, key: &SigningKey) -> Result<Vec<u8>> {
    let new_cd_offset = sections.contents.len() as u32
        + estimate_signing_block_size() as u32;
    let digest = compute_content_digest(sections, new_cd_offset)?;
    let signed_data = build_signed_data(&digest, key.certificate_der())?;
    let signature = key.sign(&signed_data)?;
    build_signer(&signed_data, &digest, &signature, key)
}

/// Build the v2 signature block value (the content of the 0x7109871a pair).
fn build_v2_block(sections: &ApkSections<'_>, key: &SigningKey) -> Result<Vec<u8>> {
    // Compute content digests over the three sections
    let new_cd_offset = sections.contents.len() as u32
        + estimate_signing_block_size() as u32;
    let digest = compute_content_digest(sections, new_cd_offset)?;

    // Build signed data
    let signed_data = build_signed_data(&digest, key.certificate_der())?;

    // Sign the signed data
    let signature = key.sign(&signed_data)?;

    // Build the signer block
    let signer = build_signer(&signed_data, &digest, &signature, key)?;

    // The v2 block is a length-prefixed sequence of signers
    let mut block = Vec::new();
    // Signers sequence (length-prefixed)
    write_length_prefixed_block(&mut block, &signer);

    Ok(block)
}

/// Estimate signing block size for CD offset calculation.
/// This is approximate — the exact size depends on signature length.
/// ECDSA P-256 signatures are ~70-72 bytes. We use a generous estimate.
fn estimate_signing_block_size() -> usize {
    // 8 (leading size) + 8 (pair length) + 4 (pair id) + ~1024 (signer data) +
    // 8 (trailing size) + 16 (magic)
    // Generous estimate to ensure correct offset
    2048
}

/// Compute the chunked content digest over the three APK sections.
///
/// The digest is a Merkle tree: each 1MB chunk is hashed with a 0xa5 prefix,
/// then all chunk digests are combined with a 0x5a prefix.
fn compute_content_digest(sections: &ApkSections<'_>, new_cd_offset: u32) -> Result<Vec<u8>> {
    // Patch EOCD to have the new CD offset for digesting
    let mut patched_eocd = sections.eocd.to_vec();
    if patched_eocd.len() >= 22 {
        let offset_bytes = new_cd_offset.to_le_bytes();
        patched_eocd[16] = offset_bytes[0];
        patched_eocd[17] = offset_bytes[1];
        patched_eocd[18] = offset_bytes[2];
        patched_eocd[19] = offset_bytes[3];
    }

    // Collect all chunk digests from all three sections
    let mut chunk_digests = Vec::new();
    digest_section_chunks(sections.contents, &mut chunk_digests);
    digest_section_chunks(sections.central_dir, &mut chunk_digests);
    digest_section_chunks(&patched_eocd, &mut chunk_digests);

    // Top-level digest: 0x5a || chunk_count (u32 LE) || all chunk digests
    let chunk_count = chunk_digests.len() as u32;
    let mut top_input = Vec::new();
    top_input.push(0x5a);
    top_input.extend_from_slice(&chunk_count.to_le_bytes());
    for cd in &chunk_digests {
        top_input.extend_from_slice(cd);
    }

    let final_digest = digest::digest(&SHA256, &top_input);
    Ok(final_digest.as_ref().to_vec())
}

/// Compute chunk digests for a single section.
fn digest_section_chunks(data: &[u8], chunk_digests: &mut Vec<Vec<u8>>) {
    if data.is_empty() {
        return;
    }

    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + CHUNK_SIZE).min(data.len());
        let chunk = &data[offset..end];
        let chunk_len = chunk.len() as u32;

        // chunk_digest = SHA256(0xa5 || chunk_length_u32_le || chunk_data)
        let mut ctx = digest::Context::new(&SHA256);
        ctx.update(&[0xa5]);
        ctx.update(&chunk_len.to_le_bytes());
        ctx.update(chunk);

        chunk_digests.push(ctx.finish().as_ref().to_vec());
        offset = end;
    }
}

/// Build the signed data blob.
///
/// Format: length-prefixed block containing:
/// - digests: sequence of (algorithm_id, digest)
/// - certificates: sequence of DER certificates
fn build_signed_data(digest: &[u8], certificate_der: &[u8]) -> Result<Vec<u8>> {
    let mut signed_data = Vec::new();

    // Digests sequence
    let mut digests = Vec::new();
    {
        let mut entry = Vec::new();
        entry.extend_from_slice(&SIG_ECDSA_SHA256.to_le_bytes()); // algorithm ID
        write_length_prefixed_bytes(&mut entry, digest); // digest value
        write_length_prefixed_block(&mut digests, &entry);
    }
    write_length_prefixed_block(&mut signed_data, &digests);

    // Certificates sequence
    let mut certs = Vec::new();
    write_length_prefixed_bytes(&mut certs, certificate_der);
    write_length_prefixed_block(&mut signed_data, &certs);

    // Additional attributes (empty)
    write_length_prefixed_block(&mut signed_data, &[]);

    Ok(signed_data)
}

/// Build a signer block.
///
/// Format: length-prefixed block containing:
/// - signed_data (length-prefixed)
/// - signatures: sequence of (algorithm_id, signature)
/// - public_key (length-prefixed, SubjectPublicKeyInfo DER)
fn build_signer(
    signed_data: &[u8],
    _digest: &[u8],
    signature: &[u8],
    key: &SigningKey,
) -> Result<Vec<u8>> {
    let mut signer = Vec::new();

    // Signed data
    write_length_prefixed_bytes(&mut signer, signed_data);

    // Signatures sequence
    let mut sigs = Vec::new();
    {
        let mut entry = Vec::new();
        entry.extend_from_slice(&SIG_ECDSA_SHA256.to_le_bytes());
        write_length_prefixed_bytes(&mut entry, signature);
        write_length_prefixed_block(&mut sigs, &entry);
    }
    write_length_prefixed_block(&mut signer, &sigs);

    // Public key (SubjectPublicKeyInfo)
    let spki = crate::der::ec_subject_public_key_info(key.public_key_bytes());
    write_length_prefixed_bytes(&mut signer, &spki);

    Ok(signer)
}

/// Write a u32 length prefix followed by data.
fn write_length_prefixed_bytes(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

/// Write a u32 length prefix followed by a block of data.
fn write_length_prefixed_block(out: &mut Vec<u8>, data: &[u8]) {
    write_length_prefixed_bytes(out, data);
}
