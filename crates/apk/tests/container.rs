// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;

use reseam_apk::{ApkFile, ApkWriteOptions, ContainerBundle, ContainerFormat};

fn lazy() -> reseam_apk::reseam_dex::ParseOptions {
    reseam_apk::reseam_dex::ParseOptions {
        lazy: true,
        ..reseam_apk::reseam_dex::ParseOptions::default()
    }
}

fn manifest_bytes(version_name: &str, split_name: Option<&str>) -> Vec<u8> {
    let split_attr = split_name
        .map(|name| format!(r#" split="{name}""#))
        .unwrap_or_default();
    reseam_apk::axml::compile_xml(
        &format!(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
        ),
        None,
    )
    .expect("compile manifest")
}

fn write_apk(path: &Path, manifest: &[u8], extra_entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("create apk");
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    writer
        .start_file("AndroidManifest.xml", options)
        .expect("manifest entry");
    writer.write_all(manifest).expect("write manifest");

    for (name, data) in extra_entries {
        writer.start_file(*name, options).expect("extra entry");
        writer.write_all(data).expect("write extra entry");
    }

    writer.finish().expect("finish apk");
}

fn write_container(path: &Path, entries: &[(&str, &[u8], zip::CompressionMethod)]) {
    let file = File::create(path).expect("create container");
    let mut writer = zip::ZipWriter::new(file);
    for (name, data, compression) in entries {
        let options = zip::write::SimpleFileOptions::default().compression_method(*compression);
        writer.start_file(*name, options).expect("container entry");
        writer.write_all(data).expect("write container entry");
    }
    writer.finish().expect("finish container");
}

fn xapk_bytes() -> Vec<u8> {
    let manifest = serde_json::json!({
        "xapk_version": 2,
        "package_name": "com.example.test",
        "version_name": "1.0",
        "split_apks": [
            { "file": "com.example.test.apk", "id": "base" },
            { "file": "config.en.apk", "id": "config.en" },
        ],
    });
    serde_json::to_vec(&manifest).expect("serialize xapk manifest")
}

fn apkm_info_bytes() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "apkm_version": 5,
        "pname": "com.example.test",
        "versioncode": "1",
        "min_api": "21",
    }))
    .expect("serialize apkm info")
}

#[test]
fn xapk_materializes_and_round_trips_through_split_set() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("split.apk");
    write_apk(
        &base,
        &manifest_bytes("1.0-base", None),
        &[("assets/base.txt", b"base")],
    );
    write_apk(
        &split,
        &manifest_bytes("1.0-split", Some("config.en")),
        &[("assets/split.txt", b"split")],
    );

    let container = tmp.path().join("app.xapk");
    write_container(
        &container,
        &[
            (
                "com.example.test.apk",
                &std::fs::read(&base).unwrap(),
                zip::CompressionMethod::Stored,
            ),
            ("icon.png", b"icon", zip::CompressionMethod::Stored),
            (
                "config.en.apk",
                &std::fs::read(&split).unwrap(),
                zip::CompressionMethod::Stored,
            ),
            (
                "manifest.json",
                &xapk_bytes(),
                zip::CompressionMethod::Stored,
            ),
        ],
    );

    let bundle = ContainerBundle::open(&container)
        .expect("open xapk")
        .expect("xapk is a container");
    assert_eq!(bundle.format(), ContainerFormat::Xapk);
    assert_eq!(bundle.package(), "com.example.test");
    assert_eq!(bundle.base_entry(), "com.example.test.apk");
    assert_eq!(bundle.split_entries(), &["config.en.apk".to_string()]);
    assert!(bundle.base_path().is_file());
    assert!(bundle.split_paths()[0].is_file());

    let mut apk = ApkFile::open_split(bundle.base_path(), bundle.split_paths(), &lazy())
        .expect("open materialized split set");
    assert_eq!(apk.components().len(), 2);
    assert_eq!(apk.components()[1].name(), "config.en");

    apk.base_mut().inject_file(
        "assets/patched.txt",
        b"patched".to_vec(),
        reseam_apk::Compression::Deflated,
    );

    let out = tmp.path().join("out");
    let written = apk
        .write_to(&out, ApkWriteOptions::default())
        .expect("write patched set");
    assert_eq!(
        written
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["com.example.test.apk", "config.en.apk"]
    );
    drop(bundle); // scratch is alive until the bundle drops
}

#[test]
fn apkm_v1_classifies_by_manifest_without_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("split_config.en.apk");
    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);
    write_apk(&split, &manifest_bytes("1.0-split", Some("config.en")), &[]);

    let container = tmp.path().join("app.apkm");
    write_container(
        &container,
        &[
            (
                "base.apk",
                &std::fs::read(&base).unwrap(),
                zip::CompressionMethod::Deflated,
            ),
            (
                "split_config.en.apk",
                &std::fs::read(&split).unwrap(),
                zip::CompressionMethod::Deflated,
            ),
        ],
    );

    let bundle = ContainerBundle::open(&container)
        .expect("open apkm")
        .expect("apkm is a container");
    assert_eq!(bundle.format(), ContainerFormat::Apkm);
    assert_eq!(bundle.package(), "com.example.test");
    assert_eq!(bundle.base_entry(), "base.apk");
    assert_eq!(bundle.split_entries(), &["split_config.en.apk".to_string()]);
}

#[test]
fn apkm_v5_reads_package_from_info_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);

    let container = tmp.path().join("app.apkm");
    write_container(
        &container,
        &[
            (
                "base.apk",
                &std::fs::read(&base).unwrap(),
                zip::CompressionMethod::Deflated,
            ),
            (
                "info.json",
                &apkm_info_bytes(),
                zip::CompressionMethod::Deflated,
            ),
            (
                "META-INF/MANIFEST.MF",
                b"Manifest-Version: 1.0",
                zip::CompressionMethod::Deflated,
            ),
            (
                "META-INF/APKMIRRO.RSA",
                b"cert",
                zip::CompressionMethod::Deflated,
            ),
        ],
    );

    let bundle = ContainerBundle::open(&container)
        .expect("open apkm")
        .expect("apkm is a container");
    assert_eq!(bundle.package(), "com.example.test");
    assert_eq!(bundle.split_entries(), &[] as &[String]);
    assert_eq!(bundle.format(), ContainerFormat::Apkm);
}

#[test]
fn plain_apk_is_not_a_container() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk = tmp.path().join("app.apk");
    write_apk(
        &apk,
        &manifest_bytes("1.0", None),
        &[("assets/x.txt", b"x")],
    );

    assert_eq!(ContainerBundle::sniff(&apk).expect("sniff"), None);
    assert!(ContainerBundle::open(&apk).expect("open").is_none());
}

#[test]
fn xapk_sniffs_by_metadata_even_when_renamed_to_apk() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);
    let container = tmp.path().join("renamed.apk");
    write_container(
        &container,
        &[
            (
                "com.example.test.apk",
                &std::fs::read(&base).unwrap(),
                zip::CompressionMethod::Stored,
            ),
            (
                "manifest.json",
                &xapk_bytes(),
                zip::CompressionMethod::Stored,
            ),
        ],
    );

    assert_eq!(
        ContainerBundle::sniff(&container).expect("sniff"),
        Some(ContainerFormat::Xapk)
    );
}

#[test]
fn container_rejects_nested_apk_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);
    let container = tmp.path().join("app.apkm");
    write_container(
        &container,
        &[(
            "nested/base.apk",
            &std::fs::read(&base).unwrap(),
            zip::CompressionMethod::Deflated,
        )],
    );

    let error = ContainerBundle::open(&container).expect_err("nested entry is rejected");
    assert!(
        error
            .to_string()
            .contains("unexpected APK entries inside folders"),
        "{error}"
    );
}

#[test]
fn container_rejects_base_flagged_as_split_by_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    // The APK's own manifest says it is a split, while manifest.json pins it as base.
    write_apk(&base, &manifest_bytes("1.0-split", Some("config.en")), &[]);
    let manifest = serde_json::json!({
        "xapk_version": 2,
        "package_name": "com.example.test",
        "split_apks": [{ "file": "config.en.apk", "id": "base" }],
    });
    let container = tmp.path().join("app.xapk");
    write_container(
        &container,
        &[
            (
                "config.en.apk",
                &std::fs::read(&base).unwrap(),
                zip::CompressionMethod::Stored,
            ),
            (
                "manifest.json",
                &serde_json::to_vec(&manifest).unwrap(),
                zip::CompressionMethod::Stored,
            ),
        ],
    );

    let error = ContainerBundle::open(&container).expect_err("mismatch is rejected");
    assert!(error.to_string().contains("no base APK"), "{error}");
}

#[test]
fn scratch_materialization_cleans_up_on_drop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);
    let container = tmp.path().join("app.apkm");
    write_container(
        &container,
        &[(
            "base.apk",
            &std::fs::read(&base).unwrap(),
            zip::CompressionMethod::Deflated,
        )],
    );

    let bundle = ContainerBundle::open(&container)
        .expect("open")
        .expect("container");
    let extracted = bundle.base_path().to_path_buf();
    assert!(extracted.parent().unwrap().is_dir());
    drop(bundle);
    assert!(!extracted.exists());
}

/// Small manifest-only fixtures keep validation tests independent of DEX parsing.
fn assert_invalid_set(
    apks: &[(&str, Vec<u8>)],
    metadata: Option<serde_json::Value>,
    expected: &str,
) {
    let tmp = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for (index, (name, manifest)) in apks.iter().enumerate() {
        let path = tmp.path().join(format!("{index}.apk"));
        write_apk(&path, manifest, &[]);
        entries.push((*name, std::fs::read(path).unwrap()));
    }
    if let Some(metadata) = metadata {
        entries.push(("manifest.json", serde_json::to_vec(&metadata).unwrap()));
    }
    let path = tmp.path().join("app.xapk");
    write_container(
        &path,
        &entries
            .iter()
            .map(|(name, data)| (*name, data.as_slice(), zip::CompressionMethod::Stored))
            .collect::<Vec<_>>(),
    );
    let error = ContainerBundle::open(&path).expect_err("invalid container must fail");
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, got {error}"
    );
}

#[test]
fn metadata_cannot_disguise_a_second_base_as_a_split() {
    assert_invalid_set(
        &[
            ("com.example.test.apk", manifest_bytes("1.0", None)),
            ("config.en.apk", manifest_bytes("1.0", None)),
        ],
        Some(serde_json::from_slice(&xapk_bytes()).unwrap()),
        "multiple base APKs",
    );
}

#[test]
fn metadata_must_describe_each_apk_once_with_its_actual_identity() {
    let base = manifest_bytes("1.0", None);
    let split = manifest_bytes("1.0", Some("config.en"));
    let original: serde_json::Value = serde_json::from_slice(&xapk_bytes()).unwrap();
    for (mapping, expected) in [
        (
            serde_json::json!([
                {"file": "com.example.test.apk", "id": "base"},
                {"file": "config.en.apk", "id": "config.en"},
                {"file": "config.en.apk", "id": "config.en"},
            ]),
            "duplicate APK reference",
        ),
        (
            serde_json::json!([
                {"file": "com.example.test.apk", "id": "base"},
                {"file": "config.en.apk", "id": "config.fr"},
            ]),
            "does not match its APK manifest",
        ),
        (
            serde_json::json!([
                {"file": "com.example.test.apk", "id": "base"},
                {"file": "missing.apk", "id": "config.en"},
            ]),
            "missing APK",
        ),
        (
            serde_json::json!([{ "file": "com.example.test.apk", "id": "base" }]),
            "unlisted APK",
        ),
    ] {
        let mut metadata = original.clone();
        metadata["split_apks"] = mapping;
        assert_invalid_set(
            &[
                ("com.example.test.apk", base.clone()),
                ("config.en.apk", split.clone()),
            ],
            Some(metadata),
            expected,
        );
    }
}

#[test]
fn splits_must_share_package_and_version_code_with_base() {
    for (package, version) in [("com.example.other", 1), ("com.example.test", 2)] {
        let manifest = reseam_apk::axml::compile_xml(&format!(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}" android:versionCode="{version}" split="config.en" />"#
        ), None).unwrap();
        // Exercise both the metadata and manifest-only paths.
        for metadata in [None, Some(serde_json::from_slice(&xapk_bytes()).unwrap())] {
            assert_invalid_set(
                &[
                    ("com.example.test.apk", manifest_bytes("1.0", None)),
                    ("config.en.apk", manifest.clone()),
                ],
                metadata,
                "does not match the base APK",
            );
        }
    }
}

#[test]
fn duplicate_split_names_are_rejected() {
    assert_invalid_set(
        &[
            ("base.apk", manifest_bytes("1.0", None)),
            ("one.apk", manifest_bytes("1.0", Some("config.en"))),
            ("two.apk", manifest_bytes("1.0", Some("config.en"))),
        ],
        None,
        "duplicate split name",
    );
}

#[test]
fn metadata_package_must_match_base() {
    assert_invalid_set(
        &[("base.apk", manifest_bytes("1.0", None))],
        Some(serde_json::json!({"package_name": "com.example.other"})),
        "metadata package does not match",
    );
}

#[test]
fn expansion_requirements_are_rejected_even_if_files_are_missing() {
    assert_invalid_set(
        &[("base.apk", manifest_bytes("1.0", None))],
        Some(serde_json::json!({
            "expansions": [{"file": "main.1.com.example.test.obb", "install_path": "Android/obb/com.example.test"}]
        })),
        "expansion files are not supported",
    );
}

#[test]
fn bundled_expansion_files_are_not_silently_discarded() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("app.xapk");
    write_container(
        &path,
        &[(
            "Android/obb/com.example.test/main.1.com.example.test.OBB",
            b"data",
            zip::CompressionMethod::Stored,
        )],
    );
    let error = ContainerBundle::open(&path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("expansion files are not supported"),
        "{error}"
    );
}

#[test]
fn extracted_names_cannot_escape_the_scratch_directory() {
    for name in [
        "../base.apk",
        "nested/base.apk",
        r"..\base.apk",
        "C:base.apk",
    ] {
        assert_invalid_set(
            &[(name, manifest_bytes("1.0", None))],
            None,
            "unexpected APK entries",
        );
    }
}

#[test]
fn apk_manifest_takes_precedence_over_embedded_container_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("app.apk");
    write_apk(
        &path,
        &manifest_bytes("1.0", None),
        &[
            ("info.json", &apkm_info_bytes()),
            ("embedded.apk", b"asset"),
        ],
    );
    assert_eq!(ContainerBundle::sniff(&path).unwrap(), None);
    assert!(ContainerBundle::open(&path).unwrap().is_none());
}

#[test]
fn extraction_checks_container_crc() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base.apk");
    write_apk(&base, &manifest_bytes("1.0", None), &[]);
    let path = tmp.path().join("app.apkm");
    write_container(
        &path,
        &[(
            "base.apk",
            &std::fs::read(base).unwrap(),
            zip::CompressionMethod::Stored,
        )],
    );
    let mut bytes = std::fs::read(&path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let offset = archive.by_name("base.apk").unwrap().data_start() as usize;
    drop(archive);
    bytes[offset] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(ContainerBundle::open(&path).is_err());
}
