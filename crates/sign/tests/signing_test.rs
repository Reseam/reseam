use stitch_sign::keystore::{GeneratedKey, SigningKey};
use stitch_sign::signing_block;
use stitch_sign::v2;

/// Create a minimal valid ZIP/APK in memory for testing.
fn create_test_apk() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("AndroidManifest.xml", options)
            .unwrap();
        writer.write_all(b"<manifest/>").unwrap();
        writer.start_file("classes.dex", options).unwrap();
        writer.write_all(&[0u8; 112]).unwrap(); // minimal DEX-sized placeholder
        writer.finish().unwrap();
    }
    buf.into_inner()
}

use std::io::Write;

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
    let loaded = SigningKey::from_pkcs8(&gen.pkcs8_der, gen.signing_key.certificate_der().to_vec())
        .unwrap();
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
    let (eocd_offset, cd_offset, _cd_size) = signing_block::find_eocd(&signed).unwrap();
    assert!(eocd_offset > 0);
    assert!(cd_offset > 0);

    // The signing block magic should be present before the central directory
    let cd_off = cd_offset as usize;
    assert!(cd_off >= 24);
    let magic = &signed[cd_off - 16..cd_off];
    assert_eq!(magic, b"APK Sig Block 42");
}

#[test]
fn test_split_apk_finds_eocd() {
    let apk = create_test_apk();
    let (eocd_offset, cd_offset, cd_size) = signing_block::find_eocd(&apk).unwrap();
    assert!(eocd_offset > 0);
    assert!(cd_offset > 0);
    assert!(cd_size > 0);
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
fn test_v3_sign() {
    let key = SigningKey::generate().unwrap();
    let apk = create_test_apk();

    let signed = stitch_sign::v3::sign(&apk, &key).unwrap();
    assert!(signed.len() > apk.len());

    // Should have signing block magic
    let (_, cd_offset, _) = signing_block::find_eocd(&signed).unwrap();
    let cd_off = cd_offset as usize;
    let magic = &signed[cd_off - 16..cd_off];
    assert_eq!(magic, b"APK Sig Block 42");
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
    assert!(archive.len() > 0);

    // Verify signing block present
    let (_, cd_offset, _) = signing_block::find_eocd(&signed).unwrap();
    let cd_off = cd_offset as usize;
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
