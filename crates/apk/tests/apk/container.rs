// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use crate::common::manifest::manifest_bytes;
use reseam_apk::{
    ApkError, ApkFile, ApkWriteOptions, Compression, ContainerFormat, open_container,
};

fn write_container(path: &Path, entries: &[(&str, &[u8])], compression: zip::CompressionMethod) {
    let options = zip::write::SimpleFileOptions::default().compression_method(compression);
    std::fs::write(path, reseam_fixtures::zip_with(entries, options)).expect("container fixture");
}

/// Each manifest's APK, also written to `dir` as `<index>.apk`.
fn fixture_apks(dir: &Path, manifests: &[(&str, Vec<u8>)]) -> Vec<Vec<u8>> {
    manifests
        .iter()
        .enumerate()
        .map(|(index, (_, manifest))| {
            let apk = reseam_fixtures::apk(manifest, &[("assets/data.txt", b"original")]);
            std::fs::write(dir.join(format!("{index}.apk")), &apk).expect("APK fixture");
            apk
        })
        .collect()
}

fn candidate_entries<'a>(
    manifests: &'a [(&'a str, Vec<u8>)],
    apks: &'a [Vec<u8>],
) -> Vec<(&'a str, &'a [u8])> {
    manifests
        .iter()
        .zip(apks)
        .map(|((name, _), bytes)| (*name, bytes.as_slice()))
        .collect()
}

#[test]
fn containers_classify_extract_and_transfer_their_lifetime_to_a_session() {
    struct Case {
        extension: &'static str,
        format: ContainerFormat,
        metadata_name: &'static str,
        metadata: &'static [u8],
    }
    for case in [
        Case { extension: "APKM", format: ContainerFormat::Apkm, metadata_name: "info.json", metadata: b"{}" },
        Case { extension: "apk", format: ContainerFormat::Apkm, metadata_name: "info.json", metadata: br#"{"apkm_version":5,"pname":"com.example.test"}"# },
        Case { extension: "xapk", format: ContainerFormat::Xapk, metadata_name: "manifest.json", metadata: br#"{"xapk_version":2,"package_name":"com.example.test","split_apks":[{"file":"base.apk","id":"base"},{"file":"config.en.apk","id":"config.en"}]}"# },
    ] {
        for compression in [zip::CompressionMethod::Stored, zip::CompressionMethod::Deflated] {
            let dir = tempfile::tempdir().expect("fixture");
            let manifests = [
                ("config.en.apk", manifest_bytes("split", Some("config.en"))),
                ("base.apk", manifest_bytes("base", None)),
            ];
            let apks = fixture_apks(dir.path(), &manifests);
            let mut entries = candidate_entries(&manifests, &apks);
            entries.push((case.metadata_name, case.metadata));
            let input = dir.path().join(format!("app.{}", case.extension));
            write_container(&input, &entries, compression);
            let (format, mut apk) = open_container(&input, ApkFile::patch_options())
                .expect("extract")
                .expect("container");
            assert_eq!(format, case.format);
            let extracted = apk.components()[0].path().to_path_buf();
            assert!(extracted.is_file());
            assert_eq!(apk.components()[0].manifest().package_name().as_deref(), Some("com.example.test"));
            assert_eq!(apk.components().len(), 2);
            assert_eq!(apk.components()[1].name(), "config.en");
            apk.inject_file(1, "assets/data.txt", b"patched split".to_vec(), Compression::Deflated).expect("patch");
            let paths = apk.write_to(dir.path().join("out"), ApkWriteOptions::default()).expect("write");
            drop(apk);
            assert!(!extracted.exists());
            let mut output = ApkFile::open_split(&paths[0], &paths[1..], ApkFile::patch_options()).expect("output");
            assert_eq!(output.read_component_entry(0, "assets/data.txt").expect("base"), Some(b"original".to_vec()));
            assert_eq!(output.read_component_entry(1, "assets/data.txt").expect("split"), Some(b"patched split".to_vec()));
        }
    }
}

#[test]
fn an_apk_manifest_takes_precedence_over_embedded_metadata_and_extension() {
    let dir = tempfile::tempdir().expect("fixture");
    for extension in ["apk", "apkm", "xapk"] {
        let path = dir.path().join(format!("app.{extension}"));
        std::fs::write(
            &path,
            reseam_fixtures::apk(
                &manifest_bytes("base", None),
                &[
                    ("info.json", br#"{"apkm_version":5}"#),
                    ("manifest.json", br#"{"xapk_version":2}"#),
                    ("embedded.apk", b"asset"),
                ],
            ),
        )
        .expect("fixture APK");
        assert!(
            open_container(&path, ApkFile::patch_options())
                .expect("open")
                .is_none()
        );
    }
}

#[test]
fn explicit_and_contained_sets_enforce_the_same_manifest_identity_rules() {
    let base = manifest_bytes("base", None);
    let split = manifest_bytes("split", Some("config.en"));
    let mismatch = |package: &str, version: u32| {
        reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}" android:versionCode="{version}" split="config.en"/>"#
    ), None).expect("manifest fixture")
    };
    for manifests in [
        vec![("one.apk", split.clone())],
        vec![("base.apk", base.clone()), ("second.apk", base.clone())],
        vec![
            ("base.apk", base.clone()),
            ("one.apk", split.clone()),
            ("two.apk", split.clone()),
        ],
        vec![
            ("base.apk", base.clone()),
            ("empty.apk", manifest_bytes("split", Some(""))),
        ],
        vec![
            ("base.apk", base.clone()),
            ("other.apk", mismatch("com.other", 1)),
        ],
        vec![
            ("base.apk", base.clone()),
            ("other.apk", mismatch("com.example.test", 2)),
        ],
        vec![(
            "base.apk",
            reseam_apk::axml::compile_xml("<manifest/>", None).expect("manifest fixture"),
        )],
    ] {
        let dir = tempfile::tempdir().expect("fixture");
        let apks = fixture_apks(dir.path(), &manifests);
        let input = dir.path().join("app.apkm");
        write_container(
            &input,
            &candidate_entries(&manifests, &apks),
            zip::CompressionMethod::Stored,
        );
        assert!(matches!(
            open_container(&input, ApkFile::patch_options()),
            Err(ApkError::Invalid { .. })
        ));
        let splits: Vec<_> = (1..manifests.len())
            .map(|index| dir.path().join(format!("{index}.apk")))
            .collect();
        assert!(matches!(
            ApkFile::open_split(dir.path().join("0.apk"), &splits, ApkFile::patch_options()),
            Err(ApkError::Invalid { .. })
        ));
    }
}

#[test]
fn container_metadata_must_describe_the_complete_set_without_overriding_manifests() {
    let manifests = [
        ("base.apk", manifest_bytes("base", None)),
        ("config.en.apk", manifest_bytes("split", Some("config.en"))),
    ];
    for metadata in [
        serde_json::json!({"package_name":"com.other"}),
        serde_json::json!({"split_apks":[{"file":"base.apk","id":"config.en"},{"file":"config.en.apk","id":"base"}]}),
        serde_json::json!({"split_apks":[{"file":"base.apk","id":"base"}]}),
        serde_json::json!({"split_apks":[{"file":"base.apk","id":"base"},{"file":"missing.apk","id":"config.en"}]}),
        serde_json::json!({"split_apks":[{"file":"base.apk","id":"base"},{"file":"config.en.apk","id":"config.en"},{"file":"config.en.apk","id":"config.en"}]}),
        serde_json::json!({"expansions":[{"file":"missing.obb"}]}),
    ] {
        let dir = tempfile::tempdir().expect("fixture");
        let apks = fixture_apks(dir.path(), &manifests);
        let metadata = serde_json::to_vec(&metadata).expect("metadata fixture");
        let mut entries = candidate_entries(&manifests, &apks);
        entries.push(("manifest.json", &metadata));
        let input = dir.path().join("app.xapk");
        write_container(&input, &entries, zip::CompressionMethod::Stored);
        assert!(matches!(
            open_container(&input, ApkFile::patch_options()),
            Err(ApkError::Invalid { .. })
        ));
    }
}

#[test]
fn unsafe_names_and_expansion_payloads_cannot_be_materialized() {
    for name in [
        "../base.apk",
        "nested/base.apk",
        r"..\base.apk",
        "C:base.apk",
        "Android/obb/main.1.app.OBB",
    ] {
        let dir = tempfile::tempdir().expect("fixture");
        let manifests = [(name, manifest_bytes("base", None))];
        let apks = fixture_apks(dir.path(), &manifests);
        let input = dir.path().join("app.apkm");
        write_container(
            &input,
            &candidate_entries(&manifests, &apks),
            zip::CompressionMethod::Stored,
        );
        assert!(matches!(
            open_container(&input, ApkFile::patch_options()),
            Err(ApkError::Invalid { .. })
        ));
    }
}

#[test]
fn extraction_checks_payload_integrity_before_opening_an_apk() {
    let dir = tempfile::tempdir().expect("fixture");
    let manifests = [("base.apk", manifest_bytes("base", None))];
    let apks = fixture_apks(dir.path(), &manifests);
    for compression in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let path = dir.path().join("app.apkm");
        write_container(&path, &[("base.apk", &apks[0])], compression);
        let mut bytes = std::fs::read(&path).expect("fixture");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("fixture");
        let offset = archive
            .by_name("base.apk")
            .expect("fixture")
            .data_start()
            .expect("offset") as usize;
        drop(archive);
        bytes[offset] ^= 1;
        std::fs::write(&path, bytes).expect("corrupt fixture");
        assert!(open_container(&path, ApkFile::patch_options()).is_err());
    }
}
