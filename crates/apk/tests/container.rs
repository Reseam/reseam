// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "common/archive.rs"]
mod archive;
#[path = "common/manifest.rs"]
mod manifest;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use archive::write_apk;
use manifest::manifest_bytes;
use reseam_apk::{
    ApkError, ApkFile, ApkWriteOptions, Compression, ContainerBundle, ContainerFormat,
};

struct Entry<'a> {
    name: &'a str,
    data: &'a [u8],
    compression: zip::CompressionMethod,
}

fn write_container(path: &Path, entries: &[Entry<'_>]) {
    let mut writer = zip::ZipWriter::new(File::create(path).expect("container fixture"));
    for entry in entries {
        writer
            .start_file(
                entry.name,
                zip::write::SimpleFileOptions::default().compression_method(entry.compression),
            )
            .expect("container entry");
        writer.write_all(entry.data).expect("container bytes");
    }
    writer.finish().expect("container fixture");
}

fn fixture_apks(dir: &Path, manifests: &[(&str, Vec<u8>)]) -> Vec<Vec<u8>> {
    manifests
        .iter()
        .enumerate()
        .map(|(index, (_, manifest))| {
            let path = dir.join(format!("{index}.apk"));
            write_apk(&path, manifest, &[("assets/data.txt", b"original")]);
            std::fs::read(path).expect("APK fixture")
        })
        .collect()
}

fn candidate_entries<'a>(manifests: &'a [(&str, Vec<u8>)], apks: &'a [Vec<u8>]) -> Vec<Entry<'a>> {
    manifests
        .iter()
        .zip(apks)
        .map(|((name, _), bytes)| Entry {
            name,
            data: bytes,
            compression: zip::CompressionMethod::Stored,
        })
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
            for entry in &mut entries { entry.compression = compression; }
            entries.push(Entry { name: case.metadata_name, data: case.metadata, compression });
            let input = dir.path().join(format!("app.{}", case.extension));
            write_container(&input, &entries);
            assert_eq!(ContainerBundle::sniff(&input).expect("sniff"), Some(case.format));
            let bundle = ContainerBundle::open(&input).expect("extract").expect("container");
            assert_eq!(bundle.package(), "com.example.test");
            assert_eq!(bundle.base_entry(), "base.apk");
            assert_eq!(bundle.split_entries(), ["config.en.apk"]);
            let extracted = bundle.base_path().to_path_buf();
            let mut apk = bundle.into_apk(ApkFile::patch_options()).expect("session");
            assert!(extracted.is_file());
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
        write_apk(
            &path,
            &manifest_bytes("base", None),
            &[
                ("info.json", br#"{"apkm_version":5}"#),
                ("manifest.json", br#"{"xapk_version":2}"#),
                ("embedded.apk", b"asset"),
            ],
        );
        assert_eq!(ContainerBundle::sniff(&path).expect("sniff"), None);
        assert!(ContainerBundle::open(&path).expect("open").is_none());
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
        write_container(&input, &candidate_entries(&manifests, &apks));
        assert!(matches!(
            ContainerBundle::open(&input),
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
        entries.push(Entry {
            name: "manifest.json",
            data: &metadata,
            compression: zip::CompressionMethod::Stored,
        });
        let input = dir.path().join("app.xapk");
        write_container(&input, &entries);
        assert!(matches!(
            ContainerBundle::open(&input),
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
        write_container(&input, &candidate_entries(&manifests, &apks));
        assert!(matches!(
            ContainerBundle::open(&input),
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
        write_container(
            &path,
            &[Entry {
                name: "base.apk",
                data: &apks[0],
                compression,
            }],
        );
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
        assert!(ContainerBundle::open(&path).is_err());
    }
}
