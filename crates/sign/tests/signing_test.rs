// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use reseam_sign::{SignError, SigningKey, signer_certificates, signing_block, v2};
use ring::digest::{self, SHA256};
use ring::signature::{ECDSA_P256_SHA256_ASN1, UnparsedPublicKey};

const CHUNK_SIZE: usize = 1 << 20;

#[expect(clippy::unwrap_used, reason = "fixture construction must succeed")]
fn apk(extra_entries: usize) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("AndroidManifest.xml", stored).unwrap();
    writer
        .write_all(b"<manifest package=\"com.example\"/>")
        .unwrap();
    writer.start_file("assets/content.bin", stored).unwrap();
    let chunk = vec![0xab; CHUNK_SIZE];
    writer.write_all(&chunk).unwrap();
    writer.write_all(&chunk).unwrap();
    writer.write_all(b"last chunk").unwrap();
    for index in 0..extra_entries {
        writer.start_file(format!("assets/data/{index:08}/a-long-localized-resource-name-for-central-directory-relocation.bin"), stored).unwrap();
    }
    writer
        .set_raw_comment(b"ZIP comment with PK\x05\x06 inside".as_slice().into())
        .expect("ZIP comment");
    writer.finish().unwrap().into_inner()
}

fn lp(data: &[u8], offset: usize) -> &[u8] {
    let len = u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("length prefix contains four bytes"),
    ) as usize;
    &data[offset + 4..offset + 4 + len]
}

#[expect(
    clippy::unwrap_used,
    reason = "the independent verifier must fail on any invalid signature"
)]
fn verify(apk: &[u8], key: &SigningKey) {
    let block = signing_block::block(apk).unwrap().unwrap();
    let value = signing_block::find_pair(block, signing_block::BLOCK_ID_V2)
        .unwrap()
        .unwrap();
    let signer = lp(lp(value, 0), 0);
    let signed_data = lp(signer, 0);
    let signatures = lp(signer, 4 + signed_data.len());
    assert_eq!(&lp(signatures, 0)[..4], &0x0201_u32.to_le_bytes());
    assert_eq!(&lp(lp(signed_data, 0), 0)[..4], &0x0201_u32.to_le_bytes());
    let signature = lp(lp(signatures, 0), 4);
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, key.public_key_bytes())
        .verify(signed_data, signature)
        .unwrap();
    let public_key = lp(signer, 8 + signed_data.len() + signatures.len());
    assert_eq!(public_key, key.public_key_der());
    assert_eq!(
        signer_certificates(apk).unwrap(),
        vec![key.certificate_der().to_vec()]
    );

    let sections = signing_block::split_apk(apk).unwrap();
    let mut eocd = sections.eocd().to_vec();
    eocd[16..20].copy_from_slice(&(sections.contents().len() as u32).to_le_bytes());
    let digests: Vec<_> = [sections.contents(), sections.central_directory(), &eocd]
        .into_iter()
        .flat_map(|section| section.chunks(CHUNK_SIZE))
        .map(|chunk| {
            let mut digest = digest::Context::new(&SHA256);
            digest.update(&[0xa5]);
            digest.update(&(chunk.len() as u32).to_le_bytes());
            digest.update(chunk);
            digest.finish()
        })
        .collect();
    let mut digest = digest::Context::new(&SHA256);
    digest.update(&[0x5a]);
    digest.update(&(digests.len() as u32).to_le_bytes());
    for chunk in &digests {
        digest.update(chunk.as_ref());
    }
    assert_eq!(lp(lp(lp(signed_data, 0), 0), 4), digest.finish().as_ref());
}

#[expect(
    clippy::unwrap_used,
    reason = "certificate fixture construction must succeed"
)]
fn large_certificate(key: &SigningKey) -> Vec<u8> {
    let pair = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(
        &key.pkcs8_der().into(),
        &rcgen::PKCS_ECDSA_P256_SHA256,
    )
    .unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    let mut extension = vec![0x04, 0x82, 0x10, 0x00];
    extension.resize(4100, 0xa5);
    params
        .custom_extensions
        .push(rcgen::CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 55555, 1],
            extension,
        ));
    params.self_signed(&pair).unwrap().der().to_vec()
}

#[test]
fn signing_and_resigning_preserve_zip_data_and_verify_cryptographically() {
    let key = SigningKey::generate().unwrap();
    let large = SigningKey::from_pkcs8(key.pkcs8_der(), large_certificate(&key)).unwrap();
    for extra_entries in [0, 9000] {
        let source = apk(extra_entries);
        assert!(signer_certificates(&source).unwrap().is_empty());
        let signed = v2::sign(&source, &key).unwrap();
        verify(&signed, &key);
        let original = signing_block::split_apk(&source).unwrap();
        let file = tempfile::tempfile().unwrap();
        let mut file_ref = &file;
        file_ref.write_all(&source).unwrap();
        for signer in [&key, &large, &key] {
            v2::sign_file_in_place(&file, signer).unwrap();
            let mut actual = Vec::new();
            file_ref.seek(SeekFrom::Start(0)).unwrap();
            file_ref.read_to_end(&mut actual).unwrap();
            verify(&actual, signer);
            let rewritten = signing_block::split_apk(&actual).unwrap();
            assert_eq!(rewritten.contents(), original.contents());
            assert_eq!(rewritten.central_directory(), original.central_directory());
            let mut archive = zip::ZipArchive::new(Cursor::new(&actual)).unwrap();
            let mut entry = archive.by_name("assets/content.bin").unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            assert_eq!(&data[..2 * CHUNK_SIZE], vec![0xab; 2 * CHUNK_SIZE]);
            assert_eq!(&data[2 * CHUNK_SIZE..], b"last chunk");
        }
    }
}

#[test]
fn keys_reject_malformed_and_mismatched_certificates() {
    let key = SigningKey::generate().unwrap();
    let other = SigningKey::generate().unwrap();
    for certificate in [vec![0xa5; 4096], other.certificate_der().to_vec()] {
        assert!(SigningKey::from_pkcs8(key.pkcs8_der(), certificate).is_err());
    }
    let mut trailing = key.certificate_der().to_vec();
    trailing.push(0);
    assert!(SigningKey::from_pkcs8(key.pkcs8_der(), trailing).is_err());
    let loaded = SigningKey::from_pkcs8(key.pkcs8_der(), key.certificate_der().to_vec()).unwrap();
    assert_eq!(loaded.public_key_bytes(), key.public_key_bytes());
    assert_eq!(loaded.certificate_der(), key.certificate_der());
    verify(&v2::sign(&apk(0), &loaded).unwrap(), &loaded);
}

#[test]
fn credentials_generate_reuse_and_refuse_to_replace_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let key_path = directory.path().join("identity.pk8");
    let cert_path = directory.path().join("identity.der");
    let key = SigningKey::load_or_generate(&key_path, &cert_path).unwrap();
    let loaded = SigningKey::load_or_generate(&key_path, &cert_path).unwrap();
    assert_eq!(loaded.public_key_bytes(), key.public_key_bytes());
    assert!(
        SigningKey::generate()
            .unwrap()
            .save(&key_path, &cert_path)
            .is_err()
    );
    assert_eq!(std::fs::read(&key_path).unwrap(), key.pkcs8_der());
    assert_eq!(std::fs::read(&cert_path).unwrap(), key.certificate_der());
    for path in [&key_path, &cert_path] {
        let removed = std::fs::read(path).unwrap();
        std::fs::remove_file(path).unwrap();
        let Err(SignError::PartialPair { missing, existing }) =
            SigningKey::load_or_generate(&key_path, &cert_path)
        else {
            panic!("a partial pair must report the missing credential");
        };
        assert_eq!(missing, *path);
        assert_eq!(
            std::fs::read(&existing).unwrap(),
            if path == &key_path {
                key.certificate_der()
            } else {
                key.pkcs8_der()
            }
        );
        assert!(!path.exists());
        std::fs::write(path, removed).unwrap();
    }
}

#[test]
fn certificate_discovery_rejects_malformed_blocks_and_uses_android_scheme_selection() {
    let key = SigningKey::generate().unwrap();
    let signed = v2::sign(&apk(0), &key).unwrap();
    let eocd = signing_block::find_eocd(&signed).unwrap();
    let start = signing_block::split_apk(&signed).unwrap().contents().len();
    let footer = eocd.central_directory_offset() as usize - 24;
    let block = signing_block::block(&signed).unwrap().unwrap();
    let v2 = signing_block::find_pair(block, signing_block::BLOCK_ID_V2)
        .unwrap()
        .unwrap();
    let padding = start + 20 + v2.len();
    for (offset, value) in [
        (start, 0_u64),
        (footer, 16),
        (footer, u64::MAX),
        (start + 8, 3),
        (start + 8, u64::MAX),
    ] {
        let mut broken = signed.clone();
        broken[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(signer_certificates(&broken).is_err());
    }
    let mut bad_offset = signed.clone();
    bad_offset[eocd.offset() + 16..eocd.offset() + 20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(signer_certificates(&bad_offset).is_err());
    assert!(v2::sign(&bad_offset, &key).is_err());
    let mut bad_signer = signed.clone();
    bad_signer[start + 20..start + 24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(signer_certificates(&bad_signer).is_err());
    let mut later_scheme = signed.clone();
    later_scheme[padding..padding + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        signer_certificates(&later_scheme).unwrap(),
        vec![key.certificate_der().to_vec()]
    );
}
