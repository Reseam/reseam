// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;

use reseam_apk::reseam_dex::{DexFile, DexHeader, DexVersion, ParseOptions};
use reseam_apk::resources::ResPackage;
use reseam_apk::{ApkFile, ApkWriteOptions, Compression, ResourceTable, StringPool};

const ALIGNMENT_DEFAULT: u64 = 4;
const ALIGNMENT_NATIVE_LIB: u64 = 16 * 1024;

fn lazy() -> ParseOptions {
    ParseOptions {
        lazy: true,
        ..ParseOptions::default()
    }
}

fn manifest_bytes(version_name: &str, split_name: Option<&str>) -> Vec<u8> {
    let split_attr = split_name
        .map(|name| format!(r#" split="{name}""#))
        .unwrap_or_default();
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
    ), None)
    .expect("compile manifest")
}

fn empty_strings() -> StringPool {
    StringPool::new(Vec::new(), true)
}

fn resource_table_bytes() -> Vec<u8> {
    ResourceTable {
        global_strings: empty_strings(),
        packages: Vec::new(),
    }
    .serialize()
    .expect("serialize resources")
}

fn mutable_resource_table_bytes() -> Vec<u8> {
    ResourceTable {
        global_strings: empty_strings(),
        packages: vec![ResPackage::new(
            0x7F,
            "com.example.test",
            empty_strings(),
            empty_strings(),
        )],
    }
    .serialize()
    .expect("serialize mutable resources")
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

fn entry_data_start_and_compression(
    apk_path: &Path,
    entry_name: &str,
) -> (u64, zip::CompressionMethod) {
    let file = File::open(apk_path).expect("open apk");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let entry = archive.by_name(entry_name).expect("entry");
    (entry.data_start(), entry.compression())
}

fn empty_dex_header(version: DexVersion) -> DexHeader {
    DexHeader {
        version,
        checksum: 0,
        signature: [0; 20],
        file_size: 0,
        link_size: 0,
        link_off: 0,
        map_off: 0,
        string_ids_size: 0,
        string_ids_off: 0,
        type_ids_size: 0,
        type_ids_off: 0,
        proto_ids_size: 0,
        proto_ids_off: 0,
        field_ids_size: 0,
        field_ids_off: 0,
        method_ids_size: 0,
        method_ids_off: 0,
        class_defs_size: 0,
        class_defs_off: 0,
        data_size: 0,
        data_off: 0,
        container_size: 0,
        header_offset: 0,
    }
}

fn minimal_dex_bytes() -> Vec<u8> {
    let dex = DexFile::new(empty_dex_header(DexVersion::V035));
    reseam_apk::reseam_dex::write(&dex).expect("write minimal dex")
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

    let mut apk = ApkFile::open_split(&base, &[split.as_path()], &lazy()).expect("open split apk");

    assert_eq!(apk.components().len(), 2);
    assert_eq!(apk.components()[1].name(), "config.test");
    assert!(apk.component_mut(1).unwrap().resources().unwrap().is_some());
    assert_eq!(
        apk.component(1)
            .unwrap()
            .manifest()
            .version_name()
            .as_deref(),
        Some("1.0-split")
    );

    apk.component_mut(1)
        .unwrap()
        .manifest_mut()
        .set_version_name("2.0-split");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write split output");

    let mut reparsed = ApkFile::open_split(
        out_dir.join("base.apk"),
        &[out_dir.join("config.apk")],
        &lazy(),
    )
    .expect("reopen split output");

    assert_eq!(
        reparsed
            .component(1)
            .unwrap()
            .manifest()
            .version_name()
            .as_deref(),
        Some("2.0-split")
    );
    assert!(reparsed
        .component_mut(1)
        .unwrap()
        .resources()
        .unwrap()
        .is_some());
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

    let mut apk = ApkFile::open_split(&base, &[split.as_path()], &lazy()).expect("open split apk");

    apk.base_mut().inject_file(
        "assets/new.txt",
        b"new-base".to_vec(),
        Compression::Deflated,
    );
    apk.base_mut().delete_file("assets/old.txt");
    apk.component_mut(1).unwrap().inject_file(
        "assets/split-only.txt",
        b"new-split".to_vec(),
        Compression::Deflated,
    );

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write split output");

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

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.base_mut().manifest_mut().set_version_name("2.0-base");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write output");

    let file = File::open(out_dir.join("signed.apk")).expect("open output");
    let archive = zip::ZipArchive::new(file).expect("zip archive");
    assert!(archive.index_for_name("META-INF/MANIFEST.MF").is_none());
    assert!(archive.index_for_name("META-INF/CERT.SF").is_none());
    assert!(archive.index_for_name("META-INF/CERT.RSA").is_none());
    assert!(archive.index_for_name("assets/data.txt").is_some());
}

#[test]
fn apk_write_aligns_passthrough_native_libraries_for_16kb_pages() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");
    let native_lib = vec![0x7F, b'E', b'L', b'F', 1, 2, 3, 4];

    write_apk(
        &apk_path,
        &manifest_bytes("1.0-base", None),
        &[
            ("assets/raw.bin", b"raw-data"),
            ("lib/arm64-v8a/libdemo.so", native_lib.as_slice()),
        ],
    );

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.base_mut().manifest_mut().set_version_name("2.0-base");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write output");

    let out_apk = out_dir.join("app.apk");
    let (asset_start, asset_compression) =
        entry_data_start_and_compression(&out_apk, "assets/raw.bin");
    let (native_start, native_compression) =
        entry_data_start_and_compression(&out_apk, "lib/arm64-v8a/libdemo.so");

    assert_eq!(asset_compression, zip::CompressionMethod::Stored);
    assert_eq!(asset_start % ALIGNMENT_DEFAULT, 0);
    assert_eq!(native_compression, zip::CompressionMethod::Stored);
    assert_eq!(native_start % ALIGNMENT_NATIVE_LIB, 0);
}

#[test]
fn native_library_injection_is_stored_and_16kb_aligned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");

    write_apk(&apk_path, &manifest_bytes("1.0-base", None), &[]);

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.base_mut().inject_file(
        "lib/arm64-v8a/libnew.so",
        vec![0x7F, b'E', b'L', b'F'],
        Compression::Deflated,
    );

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write output");

    let (native_start, native_compression) =
        entry_data_start_and_compression(&out_dir.join("app.apk"), "lib/arm64-v8a/libnew.so");

    assert_eq!(native_compression, zip::CompressionMethod::Stored);
    assert_eq!(native_start % ALIGNMENT_NATIVE_LIB, 0);
}

#[test]
fn manifest_only_write_preserves_untouched_dex_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");
    let dex_bytes = minimal_dex_bytes();
    let dex_time = zip::DateTime::from_date_and_time(2004, 5, 6, 7, 8, 10).expect("dex time");

    let file = File::create(&apk_path).expect("create apk");
    let mut writer = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(dex_time);

    writer
        .start_file("AndroidManifest.xml", stored)
        .expect("manifest entry");
    writer
        .write_all(&manifest_bytes("1.0-base", None))
        .expect("write manifest");
    writer
        .start_file("classes.dex", deflated)
        .expect("dex entry");
    writer.write_all(&dex_bytes).expect("write dex");
    writer.finish().expect("finish apk");

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.base_mut().manifest_mut().set_version_name("2.0-base");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write output");

    let file = File::open(out_dir.join("app.apk")).expect("open output");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let mut dex_entry = archive.by_name("classes.dex").expect("classes.dex");
    let mut output_dex = Vec::new();
    std::io::Read::read_to_end(&mut dex_entry, &mut output_dex).expect("read output dex");

    assert_eq!(output_dex, dex_bytes);
    assert_eq!(dex_entry.compression(), zip::CompressionMethod::Deflated);
    assert_eq!(dex_entry.last_modified(), Some(dex_time));
}

#[test]
fn split_write_preserves_untouched_dex_component_placement() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("config.apk");
    let dex_bytes = minimal_dex_bytes();

    write_apk(&base, &manifest_bytes("1.0-base", None), &[]);
    write_apk(
        &split,
        &manifest_bytes("1.0-split", Some("config.test")),
        &[("classes.dex", &dex_bytes)],
    );

    let mut apk = ApkFile::open_split(&base, &[split.as_path()], &lazy()).expect("open split apk");

    apk.component_mut(1)
        .unwrap()
        .manifest_mut()
        .set_version_name("2.0-split");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write split output");

    let base_file = File::open(out_dir.join("base.apk")).expect("open base output");
    let base_archive = zip::ZipArchive::new(base_file).expect("base zip archive");
    assert!(base_archive.index_for_name("classes.dex").is_none());

    let split_file = File::open(out_dir.join("config.apk")).expect("open split output");
    let split_archive = zip::ZipArchive::new(split_file).expect("split zip archive");
    assert!(split_archive.index_for_name("classes.dex").is_some());
}

#[test]
fn repeated_write_preserves_added_dex_without_reopen() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");

    write_apk(&apk_path, &manifest_bytes("1.0-base", None), &[]);

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.add_dex(DexFile::new(empty_dex_header(DexVersion::V035)));

    let out_one = tmp.path().join("out-one");
    apk.write_to(&out_one, ApkWriteOptions::default())
        .expect("write first output");

    let out_two = tmp.path().join("out-two");
    apk.write_to(&out_two, ApkWriteOptions::default())
        .expect("write second output");

    let file = File::open(out_two.join("app.apk")).expect("open second output");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let mut dex_entry = archive.by_name("classes.dex").expect("classes.dex");
    let mut output_dex = Vec::new();
    std::io::Read::read_to_end(&mut dex_entry, &mut output_dex).expect("read output dex");

    assert_eq!(output_dex, minimal_dex_bytes());

    let reparsed = ApkFile::open(out_two.join("app.apk"), &ParseOptions::default())
        .expect("reopen second output");
    assert_eq!(reparsed.dex().len(), 1);
}

#[test]
fn repeated_write_preserves_modified_resources_without_reopen() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");

    write_apk(
        &apk_path,
        &manifest_bytes("1.0-base", None),
        &[("resources.arsc", &mutable_resource_table_bytes())],
    );

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    let res_id = apk
        .base_mut()
        .resources_mut()
        .unwrap()
        .expect("resources")
        .add_string_resource("greeting", "hello")
        .expect("add string resource");

    let out_one = tmp.path().join("out-one");
    apk.write_to(&out_one, ApkWriteOptions::default())
        .expect("write first output");

    let out_two = tmp.path().join("out-two");
    apk.write_to(&out_two, ApkWriteOptions::default())
        .expect("write second output");

    let mut reparsed =
        ApkFile::open(out_two.join("app.apk"), &lazy()).expect("reopen second output");

    assert_eq!(
        reparsed
            .find_resource("string", "greeting")
            .unwrap()
            .map(|(_, id)| id),
        Some(res_id)
    );
    assert_eq!(
        reparsed.string_resource("greeting").unwrap().as_deref(),
        Some("hello")
    );
}

#[test]
fn manifest_only_write_preserves_untouched_resource_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let apk_path = tmp.path().join("app.apk");
    let resource_bytes = mutable_resource_table_bytes();
    let resource_time =
        zip::DateTime::from_date_and_time(2005, 6, 7, 8, 9, 10).expect("resource time");

    let file = File::create(&apk_path).expect("create apk");
    let mut writer = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(resource_time);

    writer
        .start_file("AndroidManifest.xml", stored)
        .expect("manifest entry");
    writer
        .write_all(&manifest_bytes("1.0-base", None))
        .expect("write manifest");
    writer
        .start_file("resources.arsc", deflated)
        .expect("resources entry");
    writer.write_all(&resource_bytes).expect("write resources");
    writer.finish().expect("finish apk");

    let mut apk = ApkFile::open(&apk_path, &lazy()).expect("open apk");
    apk.base_mut().manifest_mut().set_version_name("2.0-base");

    let out_dir = tmp.path().join("out");
    apk.write_to(&out_dir, ApkWriteOptions::default())
        .expect("write output");

    let file = File::open(out_dir.join("app.apk")).expect("open output");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let mut resources_entry = archive.by_name("resources.arsc").expect("resources.arsc");
    let mut output_resources = Vec::new();
    std::io::Read::read_to_end(&mut resources_entry, &mut output_resources)
        .expect("read output resources");

    assert_eq!(output_resources, resource_bytes);
    assert_eq!(
        resources_entry.compression(),
        zip::CompressionMethod::Deflated
    );
    assert_eq!(resources_entry.last_modified(), Some(resource_time));
}
