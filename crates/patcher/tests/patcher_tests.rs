// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::resources::{EntryValue, ResEntry, ResPackage, ResType, TypeSpec};
use reseam_apk::{ApkFile, ResValue, ResourceTable, StringPool};
use reseam_patcher::bundle::{BundleArchive, ENGINE_VERSION};
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, PatchSelection, PatchStatus};
use reseam_patcher::options::{OptionValue, PatchOptions};
use reseam_patcher::Patch;

static FIXTURE_JAR: OnceLock<PathBuf> = OnceLock::new();

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("patcher crate dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn run_checked(cmd: &mut Command, context: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("{context}: failed to spawn: {e}"));
    if !output.status.success() {
        panic!(
            "{context} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn build_fixture_jar() -> PathBuf {
    FIXTURE_JAR
        .get_or_init(|| {
            let root = workspace_root();
            let gradle = root.join("gradlew");
            let fixture_dir = root.join("tests/kotlin-runtime-bundle");

            run_checked(
                Command::new(&gradle)
                    .arg("-p")
                    .arg(&root)
                    .arg(":reseam-patch-sdk:jar"),
                "build reseam patch sdk jar",
            );
            run_checked(
                Command::new(&gradle).arg("-p").arg(&fixture_dir).arg("jar"),
                "build kotlin runtime test bundle",
            );

            fixture_dir.join("build/libs/reseam-test-patches.jar")
        })
        .clone()
}

const TEST_SIGNING_SEED: [u8; 32] = [0x42; 32];

struct TestBundle {
    _dir: tempfile::TempDir,
    path: PathBuf,
    pubkey: [u8; 32],
}

fn write_bundle_reseam() -> TestBundle {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;

    let tmp = tempfile::tempdir().expect("tempdir failed");
    let out_path = tmp.path().join("runtime-test-bundle.reseam");

    let jar_bytes = fs::read(build_fixture_jar()).expect("read fixture jar");
    let jar_name = "reseam-test-patches.jar";
    let jar_sha = hex::encode(Sha256::digest(&jar_bytes));

    let manifest = format!(
        r#"[bundle]
name = "runtime-test-bundle"
format_version = 1
engine = "{ENGINE_VERSION}"

[files]
"{jar_name}" = "{jar_sha}"
"#
    );
    let manifest_bytes = manifest.into_bytes();

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&TEST_SIGNING_SEED);
    let pubkey = signing_key.verifying_key().to_bytes();
    let signature = ed25519_dalek::Signer::sign(&signing_key, &manifest_bytes).to_bytes();

    let file = File::create(&out_path).expect("create .reseam");
    let mut zip = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes())
        .unwrap();
    zip.start_file("manifest.toml", deflated).unwrap();
    zip.write_all(&manifest_bytes).unwrap();
    zip.start_file("manifest.pubkey", stored).unwrap();
    zip.write_all(&pubkey).unwrap();
    zip.start_file("manifest.sig", stored).unwrap();
    zip.write_all(&signature).unwrap();
    zip.start_file(jar_name, deflated).unwrap();
    zip.write_all(&jar_bytes).unwrap();
    zip.finish().expect("finalize zip");

    TestBundle {
        _dir: tmp,
        path: out_path,
        pubkey,
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

fn resource_table_bytes(entry_name: &str, value: &str) -> Vec<u8> {
    let strings =
        |values: &[&str]| StringPool::new(values.iter().map(|s| s.to_string()).collect(), true);
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string"]),
        strings(&[entry_name]),
    );
    pkg.type_specs.push(TypeSpec::new(1, vec![0]));
    let mut t = ResType::new(1, vec![0; 48]);
    t.push(Some(ResEntry {
        flags: 0,
        key: 0,
        value: EntryValue::Simple(ResValue::string(0)),
    }));
    pkg.types.push(t);
    ResourceTable {
        global_strings: strings(&[value]),
        packages: vec![pkg],
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

fn open_split_test_apk() -> (tempfile::TempDir, ApkFile) {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let base_path = tmp.path().join("base.apk");
    let split_path = tmp.path().join("config.apk");
    let base_resources = resource_table_bytes("base_label", "Base value");
    let split_resources = resource_table_bytes("split_label", "Split original");

    write_apk(
        &base_path,
        &manifest_bytes("1.0-base", None),
        &[("resources.arsc", &base_resources)],
    );
    write_apk(
        &split_path,
        &manifest_bytes("1.0-split", Some("config.test")),
        &[("resources.arsc", &split_resources)],
    );

    let apk = ApkFile::open_split(
        &base_path,
        &[split_path.as_path()],
        &ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open split apk");

    (tmp, apk)
}

fn manifest_contains_permission(apk: &ApkFile, permission: &str) -> bool {
    let manifest = apk.base().manifest();
    (0..manifest.elements.len()).any(|i| {
        manifest.element_name(i).as_deref() == Some("uses-permission")
            && manifest
                .attribute_named(i, "name")
                .is_some_and(|attr| manifest.attribute_string(attr).as_deref() == Some(permission))
    })
}

#[test]
fn kotlin_bundle_executes_against_runtime_api() {
    let bundle_file = write_bundle_reseam();
    let archive = BundleArchive::open(&bundle_file.path).expect("open runtime bundle");
    assert_eq!(archive.public_key, bundle_file.pubkey);
    let bundle = archive.load().expect("load runtime bundle");
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);

    let mut options = PatchOptions::default();
    options.set("baseVersion", OptionValue::String("9.9-base".to_string()));
    options.set("splitVersion", OptionValue::String("9.9-split".to_string()));
    options.set(
        "splitText",
        OptionValue::String("Split patched by runtime".to_string()),
    );
    let selection = PatchSelection {
        enable: ["runtime-api", "dependent-runtime"]
            .map(String::from)
            .into(),
        options: [("runtime-api".to_string(), options)].into(),
        ..Default::default()
    };

    let results =
        engine::apply_patches(&mut ctx, &patches, &selection, |_| {}).expect("apply bundle");

    let heap = reseam_patcher::jvm_heap_stats().expect("jvm heap stats after patch run");
    assert!(heap.committed_bytes > 0, "committed heap should be nonzero");
    assert!(
        heap.used_bytes <= heap.committed_bytes,
        "used {} exceeds committed {}",
        heap.used_bytes,
        heap.committed_bytes
    );

    assert_eq!(results.len(), 4);
    let statuses = results
        .iter()
        .map(|result| (result.name.as_str(), &result.status))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(statuses.get("finalize-owner"), Some(&&PatchStatus::Applied));
    assert_eq!(statuses.get("runtime-api"), Some(&&PatchStatus::Applied));
    assert_eq!(
        statuses.get("dependent-runtime"),
        Some(&&PatchStatus::Applied)
    );
    assert!(matches!(
        statuses.get("required-option"),
        Some(&PatchStatus::Skipped { .. })
    ));

    assert_eq!(apk.version_name().as_deref(), Some("9.9-base"));
    assert_eq!(
        apk.component(1)
            .unwrap()
            .manifest()
            .version_name()
            .as_deref(),
        Some("9.9-split")
    );
    assert!(manifest_contains_permission(
        &apk,
        "android.permission.INTERNET"
    ));

    let split_label = |apk: &mut ApkFile, index: usize| {
        apk.component_mut(index)
            .unwrap()
            .resources()
            .unwrap()
            .and_then(|resources| {
                resources
                    .string_value("split_label")
                    .map(|s| s.into_owned())
            })
    };
    assert_eq!(
        split_label(&mut apk, 1).as_deref(),
        Some("Split patched by runtime")
    );
    assert_eq!(split_label(&mut apk, 0), None);

    let mut entry =
        |index: usize, name: &str| apk.component_mut(index).unwrap().read_entry(name).unwrap();
    assert_eq!(entry(0, "assets/base-marker.txt"), Some(b"base".to_vec()));
    assert_eq!(entry(1, "assets/split-marker.txt"), Some(b"split".to_vec()));
    assert_eq!(
        entry(1, "assets/dependent-marker.txt"),
        Some(b"dependent".to_vec())
    );
    assert_eq!(entry(0, "assets/split-marker.txt"), None);
}

#[test]
fn kotlin_bundle_required_option_is_enforced() {
    let bundle_file = write_bundle_reseam();
    let archive = BundleArchive::open(&bundle_file.path).expect("open runtime bundle");
    assert_eq!(archive.public_key, bundle_file.pubkey);
    let bundle = archive.load().expect("load runtime bundle");
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);

    let selection = PatchSelection {
        enable: ["required-option".to_string()].into(),
        ..Default::default()
    };

    let err = engine::apply_patches(&mut ctx, &patches, &selection, |_| {})
        .expect_err("missing required option should fail");
    let message = err.to_string();
    assert!(
        message.contains("missing required option"),
        "got: {message}"
    );
    assert!(message.contains("token"), "got: {message}");
}
