use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use stitch_apk::resources::{ResConfig, ResEntry, ResPackage, ResType, ResValue, TypeSpec};
use stitch_apk::stitch_dex::ParseOptions;
use stitch_apk::{ApkFile, AxmlEvent, ResourceTable};
use stitch_patcher::bundle::PatchBundle;
use stitch_patcher::context::PatchContext;
use stitch_patcher::engine::{self, ExecutionPlan, PatchStatus};
use stitch_patcher::options::{OptionValue, PatchOptions};

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
    let output = cmd.output().unwrap_or_else(|e| panic!("{context}: failed to spawn: {e}"));
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
            let gradle = root.join("kotlin-sdk/gradlew");
            let sdk_dir = root.join("kotlin-sdk");
            let fixture_dir = root.join("tests/kotlin-runtime-bundle");

            run_checked(
                Command::new(&gradle).arg("-p").arg(&sdk_dir).arg("jar"),
                "build stitch patch sdk jar",
            );
            run_checked(
                Command::new(&gradle).arg("-p").arg(&fixture_dir).arg("jar"),
                "build kotlin runtime test bundle",
            );

            fixture_dir.join("build/libs/stitch-test-patches.jar")
        })
        .clone()
}

const TEST_SIGNING_SEED: [u8; 32] = [0x42; 32];

struct TestBundle {
    _dir: tempfile::TempDir,
    path: PathBuf,
    pubkey: [u8; 32],
}

fn write_bundle_stitch() -> TestBundle {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;

    let tmp = tempfile::tempdir().expect("tempdir failed");
    let out_path = tmp.path().join("runtime-test-bundle.stitch");

    let jar_bytes = fs::read(build_fixture_jar()).expect("read fixture jar");
    let jar_name = "stitch-test-patches.jar";
    let jar_sha = hex::encode(Sha256::digest(&jar_bytes));

    let manifest = format!(
        r#"[bundle]
name = "runtime-test-bundle"
format_version = 1

[files]
"{jar_name}" = "{jar_sha}"
"#
    );
    let manifest_bytes = manifest.into_bytes();

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&TEST_SIGNING_SEED);
    let pubkey = signing_key.verifying_key().to_bytes();
    let signature = ed25519_dalek::Signer::sign(&signing_key, &manifest_bytes).to_bytes();

    let file = File::create(&out_path).expect("create .stitch");
    let mut zip = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(stitch_patcher::bundle::BUNDLE_MIMETYPE.as_bytes())
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
    stitch_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
    ))
    .expect("compile manifest")
}

fn resource_table_bytes(entry_name: &str, value: &str) -> Vec<u8> {
    ResourceTable {
        global_strings: vec![value.to_string()],
        packages: vec![ResPackage {
            id: 0x7F,
            name: "com.example.test".to_string(),
            type_strings: vec!["string".to_string()],
            key_strings: vec![entry_name.to_string()],
            type_specs: vec![TypeSpec {
                id: 1,
                flags: vec![0],
            }],
            types: vec![ResType {
                id: 1,
                config: ResConfig { data: vec![0; 48] },
                entries: vec![Some(ResEntry {
                    flags: 0,
                    key: 0,
                    value: ResValue::Simple {
                        data_type: 0x03,
                        data: 0,
                    },
                })],
            }],
        }],
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

    let apk = ApkFile::open_split_with_options(
        &base_path,
        &[split_path.as_path()],
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open split apk");

    (tmp, apk)
}

fn manifest_contains_permission(apk: &ApkFile, permission: &str) -> bool {
    apk.manifest().elements.iter().any(|event| {
        let AxmlEvent::StartElement {
            name,
            attributes,
            ..
        } = event
        else {
            return false;
        };
        if apk.manifest().string(*name) != Some("uses-permission") {
            return false;
        }
        attributes.iter().any(|attr| {
            apk.manifest().string(attr.name) == Some("name")
                && attr
                    .raw_value
                    .and_then(|idx| apk.manifest().string(idx))
                    == Some(permission)
        })
    })
}

#[test]
fn kotlin_bundle_executes_against_runtime_api() {
    let bundle_file = write_bundle_stitch();
    let bundle = PatchBundle::load_with_trust_anchors(&bundle_file.path, &[bundle_file.pubkey])
        .expect("load runtime bundle");
    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);

    let mut plan = ExecutionPlan::new();
    plan.select_patch("runtime-api");
    plan.select_patch("dependent-runtime");

    let mut options = PatchOptions::new();
    options.set("baseVersion", OptionValue::String("9.9-base".to_string()));
    options.set("splitVersion", OptionValue::String("9.9-split".to_string()));
    options.set(
        "splitText",
        OptionValue::String("Split patched by runtime".to_string()),
    );
    plan.set_patch_options("runtime-api", options);

    let results =
        engine::apply_patches_with_plan(&mut ctx, &bundle.patches, &plan).expect("apply bundle");

    assert_eq!(results.len(), 4);
    let statuses = results
        .iter()
        .map(|result| (result.name.as_str(), &result.status))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(statuses.get("finalize-owner"), Some(&&PatchStatus::Applied));
    assert_eq!(statuses.get("runtime-api"), Some(&&PatchStatus::Applied));
    assert_eq!(statuses.get("dependent-runtime"), Some(&&PatchStatus::Applied));
    assert!(matches!(
        statuses.get("required-option"),
        Some(&PatchStatus::Skipped { .. })
    ));

    assert_eq!(apk.version_name(), Some("9.9-base"));
    assert_eq!(
        apk.component_manifest(1).and_then(|manifest| manifest.version_name()),
        Some("9.9-split")
    );
    assert!(manifest_contains_permission(&apk, "android.permission.INTERNET"));

    assert_eq!(
        apk.component_resources(1)
            .and_then(|resources| resources.get_string_value("split_label")),
        Some("Split patched by runtime")
    );
    assert_eq!(
        apk.resources()
            .and_then(|resources| resources.get_string_value("split_label")),
        None
    );

    assert_eq!(
        apk.read_entry_from_component(0, "assets/base-marker.txt")
            .expect("base marker"),
        b"base".to_vec()
    );
    assert_eq!(
        apk.read_entry_from_component(1, "assets/split-marker.txt")
            .expect("split marker"),
        b"split".to_vec()
    );
    assert_eq!(
        apk.read_entry_from_component(1, "assets/dependent-marker.txt")
            .expect("dependent marker"),
        b"dependent".to_vec()
    );
    assert!(apk.read_entry_from_component(0, "assets/split-marker.txt").is_err());
}

#[test]
fn kotlin_bundle_required_option_is_enforced() {
    let bundle_file = write_bundle_stitch();
    let bundle = PatchBundle::load_with_trust_anchors(&bundle_file.path, &[bundle_file.pubkey])
        .expect("load runtime bundle");
    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);

    let mut plan = ExecutionPlan::new();
    plan.select_patch("required-option");

    let err = engine::apply_patches_with_plan(&mut ctx, &bundle.patches, &plan)
        .expect_err("missing required option should fail");
    let message = err.to_string();
    assert!(message.contains("missing required option"), "got: {message}");
    assert!(message.contains("token"), "got: {message}");
}
