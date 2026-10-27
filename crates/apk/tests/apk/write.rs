// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::minimal_dex_bytes;
use crate::common::manifest::manifest_bytes;
use crate::common::session::*;
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression};
use std::fs::File;

#[test]
fn write_to_applies_the_signature_policy_without_removing_other_metadata() -> TestResult {
    use reseam_apk::SignaturePolicy;
    let entries: Vec<_> = SIGNATURES
        .iter()
        .chain(&METADATA)
        .map(|&name| (name, b"payload".as_slice()))
        .collect();
    let Session { dir, mut apk } = Session::single(&entries);
    apk.base_mut().manifest_mut().set_version_name("edited");
    for (policy, retained) in [
        (SignaturePolicy::Strip, false),
        (SignaturePolicy::Preserve, true),
    ] {
        let output = apk.write_to(
            dir.path().join("out"),
            WriteOptions {
                signatures: policy,
                ..Default::default()
            },
        )?;
        let mut reopened = Apk::open(&output[0], lazy())?;
        for name in SIGNATURES.iter().chain(&METADATA) {
            assert_eq!(
                reopened.read_entry(name)?.as_deref(),
                (retained || METADATA.contains(name)).then_some(b"payload".as_slice()),
                "{name}"
            );
        }
    }
    Ok(())
}

#[test]
fn passthrough_and_injected_native_libraries_are_stored_aligned_and_unchanged() -> TestResult {
    let Session { dir, mut apk } = Session::single(&[
        ("assets/raw.bin", b"asset"),
        ("lib/arm64-v8a/liboriginal.so", b"\x7fELF original"),
    ]);
    for name in ["lib/arm64-v8a/libinjected.so", "lib/arm64-v8a/.so"] {
        apk.inject_file(0, name, b"\x7fELF injected".to_vec(), Compression::Deflated)?;
    }
    let output = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
    let mut reopened = Apk::open(&output[0], lazy())?;
    for (name, alignment, bytes) in LIBRARIES {
        let (start, compression) = entry_data_start_and_compression(&output[0], name);
        assert_eq!(start % alignment, 0, "{name}");
        assert_eq!(compression, zip::CompressionMethod::Stored);
        assert_eq!(reopened.read_entry(name)?.as_deref(), Some(bytes));
    }
    Ok(())
}

#[test]
fn manifest_edits_preserve_untouched_payload_bytes_and_archive_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("app.apk");
    let entries = [
        ("classes.dex", minimal_dex_bytes()),
        ("resources.arsc", mutable_resource_table_bytes()),
    ];
    let time = zip::DateTime::from_date_and_time(2004, 5, 6, 7, 8, 10)?;
    for compression in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .last_modified_time(time);
        let manifest = manifest_bytes("base", None);
        let packed: Vec<_> = std::iter::once(("AndroidManifest.xml", manifest.as_slice()))
            .chain(
                entries
                    .iter()
                    .map(|(name, bytes)| (*name, bytes.as_slice())),
            )
            .collect();
        std::fs::write(&path, reseam_fixtures::zip_with(&packed, options))?;
        let mut apk = Apk::open(&path, lazy())?;
        apk.base_mut().manifest_mut().set_version_name("edited");
        let output = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
        let mut zip = zip::ZipArchive::new(File::open(&output[0])?)?;
        for (name, expected) in &entries {
            let mut entry = zip.by_name(name)?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            assert_eq!(bytes, *expected);
            assert_eq!(entry.compression(), compression);
            assert_eq!(entry.last_modified(), Some(time));
        }
    }
    Ok(())
}

#[test]
fn serialization_failure_preserves_destinations_and_allows_repair() -> TestResult {
    for workers in [1, 4] {
        for invalid_component in [0, 1] {
            let entries = [("classes.dex", minimal_dex_bytes())];
            let entries = entries
                .each_ref()
                .map(|(name, data)| (*name, data.as_slice()));
            let Session { dir, mut apk } = Session::split(&entries, &entries, lazy());
            let output = dir.path().join("out");
            std::fs::create_dir(&output)?;
            let mut before = Vec::new();
            for name in ["base.apk", "split.apk"] {
                std::fs::copy(dir.path().join(name), output.join(name))?;
                before.push(std::fs::read(output.join(name))?);
            }
            apk.base_mut().manifest_mut().set_version_name("edited");
            for index in 0..2 {
                apk.dex_mut(index).expect("DEX").intern_string("edited");
            }
            for _ in 0..2 {
                apk.dex_mut(invalid_component).expect("DEX").create_class(
                    "LRepeated;",
                    reseam_dex::AccessFlags::PUBLIC,
                    None,
                )?;
            }
            let options = WriteOptions {
                dex_workers: std::num::NonZeroUsize::new(workers).expect("positive workers"),
                ..Default::default()
            };
            assert!(apk.write_to(&output, options).is_err());
            for (name, bytes) in ["base.apk", "split.apk"].into_iter().zip(before) {
                assert_eq!(std::fs::read(output.join(name))?, bytes);
            }
            let invalid = apk.dex_mut(invalid_component).expect("DEX");
            let repeated = invalid.intern_type("LRepeated;")?;
            invalid.remove_class(repeated)?.expect("duplicate class");
            let paths = apk.write_to(&output, options)?;
            let repaired = Apk::open_split(&paths[0], &paths[1..], lazy())?;
            assert_eq!(repaired.version_name().as_deref(), Some("edited"));
            assert_eq!(
                repaired
                    .dex()
                    .get(invalid_component)
                    .expect("DEX")
                    .classes()
                    .len(),
                1
            );
            assert!(
                repaired
                    .dex()
                    .iter()
                    .all(|d| d.strings().find("edited").is_some())
            );
        }
    }
    Ok(())
}
