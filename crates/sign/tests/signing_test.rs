// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_sign::signing_block;
use reseam_sign::v2;
use reseam_sign::{GeneratedKey, SigningKey};

/// Create a minimal valid ZIP/APK in memory for testing.
fn create_test_apk() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("AndroidManifest.xml", options).unwrap();
        writer.write_all(b"<manifest/>").unwrap();
        writer.start_file("classes.dex", options).unwrap();
        writer.write_all(&[0u8; 112]).unwrap(); // minimal DEX-sized placeholder
        writer.finish().unwrap();
    }
    buf.into_inner()
}

use ring::digest::{self, SHA256};
use std::io::Write;

const SIG_ECDSA_SHA256: u32 = 0x0201;
const CHUNK_SIZE: usize = 1 << 20;

fn read_lp(data: &[u8], offset: usize) -> &[u8] {
    let len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    &data[offset + 4..offset + 4 + len]
}

fn find_signing_pair_value(signing_block_bytes: &[u8], id: u32) -> Option<&[u8]> {
    if signing_block_bytes.len() < 32 {
        return None;
    }

    let mut pos = 8;
    let end = signing_block_bytes.len() - 24;
    while pos < end {
        let pair_len =
            u64::from_le_bytes(signing_block_bytes[pos..pos + 8].try_into().ok()?) as usize;
        let pair_end = pos.checked_add(8 + pair_len)?;
        if pair_end > end {
            return None;
        }

        let pair_id = u32::from_le_bytes(signing_block_bytes[pos + 8..pos + 12].try_into().ok()?);
        if pair_id == id {
            return Some(&signing_block_bytes[pos + 12..pair_end]);
        }

        pos = pair_end;
    }

    None
}

fn extract_v2_digest(signed_apk: &[u8]) -> Vec<u8> {
    let sections = signing_block::split_apk(signed_apk).unwrap();
    let cd_offset = signing_block::find_eocd(signed_apk).unwrap().cd_offset as usize;
    let signing_block_bytes = &signed_apk[sections.contents.len()..cd_offset];
    let v2_block =
        find_signing_pair_value(signing_block_bytes, signing_block::BLOCK_ID_V2).unwrap();
    let signers_seq = read_lp(v2_block, 0);
    let signer = read_lp(signers_seq, 0);
    let signed_data = read_lp(signer, 0);
    let digests_seq = read_lp(signed_data, 0);
    let digest_entry = read_lp(digests_seq, 0);
    let algorithm_id = u32::from_le_bytes(digest_entry[0..4].try_into().unwrap());
    assert_eq!(algorithm_id, SIG_ECDSA_SHA256);
    read_lp(digest_entry, 4).to_vec()
}

fn compute_content_digest(
    contents: &[u8],
    central_dir: &[u8],
    eocd: &[u8],
    new_cd_offset: u32,
) -> Vec<u8> {
    let mut patched_eocd = eocd.to_vec();
    patched_eocd[16..20].copy_from_slice(&new_cd_offset.to_le_bytes());

    let mut chunk_digests = Vec::new();
    digest_section_chunks(contents, &mut chunk_digests);
    digest_section_chunks(central_dir, &mut chunk_digests);
    digest_section_chunks(&patched_eocd, &mut chunk_digests);

    let mut top_input = vec![0x5a];
    top_input.extend_from_slice(&(chunk_digests.len() as u32).to_le_bytes());
    for digest in &chunk_digests {
        top_input.extend_from_slice(digest);
    }

    digest::digest(&SHA256, &top_input).as_ref().to_vec()
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

#[test]
fn test_key_generation() {
    let key = SigningKey::generate().unwrap();
    assert!(!key.certificate_der().is_empty());
    assert!(!key.public_key_bytes().is_empty());
}

#[test]
fn test_generated_key_save_load() {
    let gen = GeneratedKey::generate().unwrap();
    assert!(!gen.pkcs8_der.is_empty());

    // Round-trip: load from saved PKCS#8 bytes
    let loaded =
        SigningKey::from_pkcs8(&gen.pkcs8_der, gen.signing_key.certificate_der().to_vec()).unwrap();
    assert_eq!(
        loaded.public_key_bytes(),
        gen.signing_key.public_key_bytes()
    );
}

#[test]
fn test_sign_and_verify_structure() {
    let key = SigningKey::generate().unwrap();
    let apk = create_test_apk();

    let signed = v2::sign(&apk, &key).unwrap();

    // Signed APK should be larger (signing block added)
    assert!(signed.len() > apk.len());

    // Should still be a valid ZIP (EOCD present)
    let eocd = signing_block::find_eocd(&signed).unwrap();
    assert!(eocd.offset > 0);
    assert!(eocd.cd_offset > 0);

    // The signing block magic should be present before the central directory
    let cd_off = eocd.cd_offset as usize;
    assert!(cd_off >= 24);
    let magic = &signed[cd_off - 16..cd_off];
    assert_eq!(magic, b"APK Sig Block 42");
}

#[test]
fn test_split_apk_finds_eocd() {
    let apk = create_test_apk();
    let eocd = signing_block::find_eocd(&apk).unwrap();
    assert!(eocd.offset > 0);
    assert!(eocd.cd_offset > 0);
    assert!(eocd.cd_size > 0);
}

#[test]
fn test_split_apk_sections() {
    let apk = create_test_apk();
    let sections = signing_block::split_apk(&apk).unwrap();
    assert!(!sections.contents.is_empty());
    assert!(!sections.central_dir.is_empty());
    assert!(!sections.eocd.is_empty());
}

#[test]
fn test_find_eocd_rejects_trailing_bytes() {
    let mut apk = create_test_apk();
    apk.extend_from_slice(b"junk");
    assert!(signing_block::find_eocd(&apk).is_err());
}

#[test]
fn test_signed_apk_still_valid_zip() {
    let key = SigningKey::generate().unwrap();
    let apk = create_test_apk();

    let signed = v2::sign(&apk, &key).unwrap();

    // Should be parseable as a ZIP
    let cursor = std::io::Cursor::new(&signed);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    assert!(archive.len() >= 2); // AndroidManifest.xml + classes.dex

    // Verify entries are still readable
    let mut manifest = archive.by_name("AndroidManifest.xml").unwrap();
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut manifest, &mut content).unwrap();
    assert_eq!(content, b"<manifest/>");
}

#[test]
fn test_sign_large_certificate_uses_final_cd_offset_in_digest() {
    let generated = GeneratedKey::generate().unwrap();
    let oversized_cert = vec![0xA5; 4096];
    let key = SigningKey::from_pkcs8(&generated.pkcs8_der, oversized_cert).unwrap();
    let apk = create_test_apk();

    let signed = v2::sign(&apk, &key).unwrap();
    let stored_digest = extract_v2_digest(&signed);

    let sections = signing_block::split_apk(&signed).unwrap();
    let recomputed = compute_content_digest(
        sections.contents,
        sections.central_dir,
        sections.eocd,
        sections.contents.len() as u32,
    );

    assert_eq!(stored_digest, recomputed);
}

#[test]
fn test_sign_real_apk() {
    let apk_path = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
    if !std::path::Path::new(apk_path).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let apk = std::fs::read(apk_path).unwrap();
    let key = SigningKey::generate().unwrap();

    let signed = v2::sign(&apk, &key).unwrap();

    // Verify ZIP still valid
    let cursor = std::io::Cursor::new(&signed);
    let archive = zip::ZipArchive::new(cursor).unwrap();
    assert!(!archive.is_empty());

    // Verify signing block present
    let cd_off = signing_block::find_eocd(&signed).unwrap().cd_offset as usize;
    let magic = &signed[cd_off - 16..cd_off];
    assert_eq!(magic, b"APK Sig Block 42");

    let diff = signed.len() as i64 - apk.len() as i64;
    eprintln!(
        "Signed APK: {} -> {} bytes ({:+})",
        apk.len(),
        signed.len(),
        diff
    );
}

#[test]
fn test_sign_in_place_matches_sign() {
    let apk = create_test_apk();
    let key = SigningKey::generate().unwrap();
    let expected = v2::sign(&apk, &key).unwrap();

    let file = tempfile::tempfile().unwrap();
    {
        use std::io::Write;
        let mut writer = &file;
        writer.write_all(&apk).unwrap();
    }
    v2::sign_file_in_place(&file, &key).unwrap();
    let mut signed = Vec::new();
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut reader = &file;
        reader.seek(SeekFrom::Start(0)).unwrap();
        reader.read_to_end(&mut signed).unwrap();
    }
    // ECDSA signatures differ in length per signing and the block padding
    // absorbs that, so compare everything around the signing block.
    let contents_len = signing_block::split_apk(&apk).unwrap().contents.len();
    let tail_len = apk.len() - contents_len;
    assert_eq!(signed.len(), expected.len());
    assert_eq!(extract_v2_digest(&signed), extract_v2_digest(&expected));
    assert_eq!(signed[..contents_len], expected[..contents_len]);
    assert_eq!(
        signed[signed.len() - tail_len..],
        expected[expected.len() - tail_len..]
    );
    assert!(signing_block::split_apk(&signed).is_ok());
}
