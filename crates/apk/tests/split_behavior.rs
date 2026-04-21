// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;

use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ResourceTable};

fn manifest_bytes(version_name: &str, split_name: Option<&str>) -> Vec<u8> {
    let split_attr = split_name
        .map(|name| format!(r#" split="{name}""#))
        .unwrap_or_default();
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
    ))
    .expect("compile manifest")
}

fn resource_table_bytes() -> Vec<u8> {
    ResourceTable {
        global_strings: Vec::new(),
        global_strings_utf8: true,
        packages: Vec::new(),
    }
    .serialize()
    .expect("serialize resources")
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

#[test]
fn split_apk_supports_split_resource_tables_and_component_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("config.apk");

    write_apk(
        &base,
        &manifest_bytes("1.0-base", None),
        &[("assets/base.txt", b"base")],
    );
    write_apk(
        &split,
        &manifest_bytes("1.0-split", Some("config.test")),
        &[("resources.arsc", &resource_table_bytes())],
    );

    let mut apk = ApkFile::open_split_with_options(
        &base,
        &[split.as_path()],
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open split apk");

    assert_eq!(apk.component_count(), 2);
    assert_eq!(apk.split_names(), vec!["config.test"]);
    assert!(apk.component_resources(1).is_some());
    assert_eq!(
        apk.component_manifest(1)
            .and_then(|manifest| manifest.version_name()),
        Some("1.0-split")
    );

    apk.component_manifest_mut(1)
        .expect("split manifest")
        .set_version_name("2.0-split");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir).expect("write split output");

    let reparsed = ApkFile::open_split_with_options(
        out_dir.join("base.apk"),
        &[out_dir.join("config.apk")],
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("reopen split output");

    assert_eq!(
        reparsed
            .component_manifest(1)
            .and_then(|manifest| manifest.version_name()),
        Some("2.0-split")
    );
    assert!(reparsed.component_resources(1).is_some());
}

#[test]
fn split_apk_file_changes_are_component_scoped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("config.apk");

    write_apk(
        &base,
        &manifest_bytes("1.0-base", None),
        &[("assets/old.txt", b"old-base")],
    );
    write_apk(
        &split,
        &manifest_bytes("1.0-split", Some("config.test")),
        &[("assets/old.txt", b"old-split")],
    );

    let mut apk = ApkFile::open_split_with_options(
        &base,
        &[split.as_path()],
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open split apk");

    apk.inject_file("assets/new.txt", b"new-base".to_vec());
    apk.delete_file("assets/old.txt");
    apk.inject_file_into(1, "assets/split-only.txt", b"new-split".to_vec());

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir).expect("write split output");

    let base_file = File::open(out_dir.join("base.apk")).expect("open base output");
    let base_archive = zip::ZipArchive::new(base_file).expect("base zip archive");
    assert!(base_archive.index_for_name("assets/new.txt").is_some());
    assert!(base_archive.index_for_name("assets/old.txt").is_none());
    assert!(base_archive
        .index_for_name("assets/split-only.txt")
        .is_none());

    let split_file = File::open(out_dir.join("config.apk")).expect("open split output");
    let split_archive = zip::ZipArchive::new(split_file).expect("split zip archive");
    assert!(split_archive.index_for_name("assets/new.txt").is_none());
    assert!(split_archive.index_for_name("assets/old.txt").is_some());
    assert!(split_archive
        .index_for_name("assets/split-only.txt")
        .is_some());
}

#[test]
fn write_to_strips_stale_signature_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("signed.apk");

    write_apk(
        &apk_path,
        &manifest_bytes("1.0-base", None),
        &[
            ("META-INF/MANIFEST.MF", b"manifest"),
            ("META-INF/CERT.SF", b"sf"),
            ("META-INF/CERT.RSA", b"rsa"),
            ("assets/data.txt", b"payload"),
        ],
    );

    let mut apk = ApkFile::open_with_options(
        &apk_path,
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open apk");
    apk.manifest_mut().set_version_name("2.0-base");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir).expect("write output");

    let file = File::open(out_dir.join("signed.apk")).expect("open output");
    let archive = zip::ZipArchive::new(file).expect("zip archive");
    assert!(archive.index_for_name("META-INF/MANIFEST.MF").is_none());
    assert!(archive.index_for_name("META-INF/CERT.SF").is_none());
    assert!(archive.index_for_name("META-INF/CERT.RSA").is_none());
    assert!(archive.index_for_name("assets/data.txt").is_some());
}
