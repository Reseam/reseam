// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use reseam_apk::reseam_dex::{DexFile, DexHeader, DexVersion, ParseOptions};
use reseam_apk::resources::{EntryValue, MapEntry, ResEntry, ResPackage, ResType, TypeSpec};
use reseam_apk::{ApkFile, ResValue, ResourceTable, StringPool};
use reseam_patcher::Patch;
use reseam_patcher::bundle::{BundleArchive, ENGINE_VERSION};
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, PatchSelection, PatchStatus};
use reseam_patcher::options::OptionValue;

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

/// A base manifest naming `class` as the app's `Application`.
fn manifest_with_application(class: &str) -> Vec<u8> {
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="1.0-base"><application android:name="{class}" /></manifest>"#
    ), None)
    .expect("compile manifest")
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

fn strings(values: &[&str]) -> StringPool {
    StringPool::new(values.iter().map(|s| s.to_string()).collect(), true)
}

fn resource_table_bytes(entry_name: &str, value: &str) -> Vec<u8> {
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

/// The path aapt gave the base component's one layout. Release builds obfuscate
/// resource file names, which is why a patch has to resolve the entry to reach
/// the file at all.
const LAYOUT_PATH: &str = "res/Qy.xml";

const LAYOUT_XML: &str = r#"<androidx.constraintlayout.widget.ConstraintLayout
    xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:app="http://schemas.android.com/apk/res-auto"
    android:id="@+id/controls"
    android:layout_width="match_parent">
    <ImageView android:id="@+id/fullscreen_button" android:layout_width="48dp" />
</androidx.constraintlayout.widget.ConstraintLayout>"#;

/// The base component's table, with the entry shapes the resource API works on:
/// a file-backed `layout`, `attr` entries the app declares itself, a style, an
/// array, and a `mipmap` type that ships only a density configuration.
fn base_resource_table() -> ResourceTable {
    const ARRAY_FIRST_NAME: u32 = 0x0100_0001;
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string", "attr", "layout", "style", "array", "mipmap"]),
        strings(&[
            "base_label",
            "layout_constraintRight_toLeftOf",
            "player_controls",
            "Theme.Test",
            "double_tap_lengths",
            "launcher",
        ]),
    );
    let mut push = |type_id: u8, config: Vec<u8>, key: u32, value: EntryValue| {
        pkg.type_specs.push(TypeSpec::new(type_id, vec![0]));
        let mut res_type = ResType::new(type_id, config);
        res_type.push(Some(ResEntry {
            flags: 0,
            key,
            value,
        }));
        pkg.types.push(res_type);
    };
    let default = || vec![0u8; 48];
    push(1, default(), 0, EntryValue::Simple(ResValue::string(0)));
    push(2, default(), 1, EntryValue::Simple(ResValue::new(0, 0)));
    push(3, default(), 2, EntryValue::Simple(ResValue::string(1)));
    push(
        4,
        default(),
        3,
        EntryValue::Complex {
            parent: 0,
            entries: vec![MapEntry {
                name: 0x0101_0098,
                value: ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff00_0000),
            }],
        },
    );
    push(
        5,
        default(),
        4,
        EntryValue::Complex {
            parent: 0,
            entries: (0..2)
                .map(|i| MapEntry {
                    name: ARRAY_FIRST_NAME + i,
                    value: ResValue::int(5 * (i as i32 + 1)),
                })
                .collect(),
        },
    );
    let mut density = default();
    density[14..16].copy_from_slice(&480u16.to_le_bytes());
    push(6, density, 5, EntryValue::Simple(ResValue::string(2)));
    ResourceTable {
        global_strings: strings(&["Base value", LAYOUT_PATH, "res/launcher.png"]),
        packages: vec![pkg],
    }
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

fn open_split_test_apk() -> (tempfile::TempDir, ApkFile) {
    open_split_test_apk_with(manifest_bytes("1.0-base", None), &[])
}

/// The app's own code belongs in the base APK as `classes*.dex`. A DEX added
/// after opening is one the patcher linked in, which is what tells the app's
/// classes apart from a bundle extension's.
fn open_split_test_apk_with(
    base_manifest: Vec<u8>,
    dex_files: &[DexFile],
) -> (tempfile::TempDir, ApkFile) {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let base_path = tmp.path().join("base.apk");
    let split_path = tmp.path().join("config.apk");
    let mut base_table = base_resource_table();
    // Compiling against the table creates the `@+id` entries the layout and the
    // fixture patches refer to, so the table is serialized after it.
    let layout = reseam_apk::axml::compile_xml(LAYOUT_XML, Some(&mut base_table))
        .expect("compile the base layout");
    let base_resources = base_table.serialize().expect("serialize resources");
    let split_resources = resource_table_bytes("split_label", "Split original");

    let dex_bytes: Vec<(String, Vec<u8>)> = dex_files
        .iter()
        .enumerate()
        .map(|(index, dex)| {
            let name = match index {
                0 => "classes.dex".to_string(),
                n => format!("classes{}.dex", n + 1),
            };
            (name, reseam_apk::reseam_dex::write(dex).expect("write dex"))
        })
        .collect();
    let mut base_entries: Vec<(&str, &[u8])> =
        vec![("resources.arsc", &base_resources), (LAYOUT_PATH, &layout)];
    base_entries.extend(
        dex_bytes
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice())),
    );

    write_apk(&base_path, &base_manifest, &base_entries);
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

    let mut options = std::collections::HashMap::new();
    options.insert(
        "baseVersion".to_owned(),
        OptionValue::Text("9.9-base".to_string()),
    );
    options.insert(
        "splitVersion".to_owned(),
        OptionValue::Text("9.9-split".to_string()),
    );
    options.insert(
        "splitText".to_owned(),
        OptionValue::Text("Split patched by runtime".to_string()),
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

    assert_eq!(results.len(), patches.len());
    let statuses = results
        .iter()
        .map(|result| (result.patch.as_str(), &result.status))
        .collect::<std::collections::HashMap<_, _>>();
    let internal = patches
        .iter()
        .find(|patch| patch.reference() == "runtime-test-bundle/app.reseam.test.internalHelper")
        .expect("the fixture declares an internal patch");
    assert_eq!(
        internal.reference(),
        "runtime-test-bundle/app.reseam.test.internalHelper"
    );
    assert_eq!(internal.spec().name, "app.reseam.test.internalHelper");
    assert!(!internal.spec().enabled_by_default);
    assert!(matches!(
        statuses.get(internal.reference().as_str()),
        Some(&PatchStatus::Skipped { .. })
    ));
    assert_eq!(
        statuses.get("runtime-test-bundle/app.reseam.test.finalizeOwner"),
        Some(&&PatchStatus::Applied)
    );
    assert_eq!(
        statuses.get("runtime-test-bundle/app.reseam.test.runtimeApi"),
        Some(&&PatchStatus::Applied)
    );
    assert_eq!(
        statuses.get("runtime-test-bundle/app.reseam.test.dependentRuntime"),
        Some(&&PatchStatus::Applied)
    );
    assert!(matches!(
        statuses.get("runtime-test-bundle/app.reseam.test.requiredOption"),
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
fn internal_patches_run_as_dependencies() {
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .expect("open runtime bundle")
        .load()
        .expect("load runtime bundle");
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);

    let selection = PatchSelection {
        enable: ["uses-internal".to_string()].into(),
        ..Default::default()
    };
    let results =
        engine::apply_patches(&mut ctx, &patches, &selection, |_| {}).expect("apply bundle");
    let applied: Vec<&str> = results
        .iter()
        .filter(|result| result.status == PatchStatus::Applied)
        .map(|result| result.patch.as_str())
        .collect();
    assert_eq!(
        applied,
        [
            "runtime-test-bundle/app.reseam.test.internalHelper",
            "runtime-test-bundle/app.reseam.test.usesInternal"
        ]
    );
    let helper = results
        .iter()
        .find(|result| result.patch == "runtime-test-bundle/app.reseam.test.internalHelper")
        .expect("the internal helper ran");
    assert!(helper.hidden);
    assert_eq!(
        helper.required_by,
        ["runtime-test-bundle/app.reseam.test.usesInternal"]
    );
    let dependent = results
        .iter()
        .find(|result| result.patch == "runtime-test-bundle/app.reseam.test.usesInternal")
        .expect("the selected patch ran");
    assert!(!dependent.hidden);
    assert!(
        dependent.required_by.is_empty(),
        "it was asked for directly"
    );
    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/internal-marker.txt")
            .unwrap(),
        Some(b"internal".to_vec())
    );
}

#[test]
fn a_patch_whose_bundle_is_missing_skips_unless_selected() {
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .expect("open runtime bundle")
        .load()
        .expect("load runtime bundle");
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    assert!(
        !patches
            .iter()
            .any(|patch| patch.reference().contains("otherBundleHelper")),
        "an ExternalPatch is a reference, not a declaration"
    );
    let results = engine::validate_patches(
        &patches,
        &PatchSelection::default(),
        Some("com.example.test"),
        None,
    )
    .unwrap();
    let skipped = results
        .iter()
        .find(|result| result.patch == "runtime-test-bundle/app.reseam.test.needsOtherBundle")
        .expect("the fixture declares a patch needing another bundle");
    assert_eq!(
        skipped.status,
        PatchStatus::Skipped {
            reason: "depends on other-bundle/app.reseam.other.helper; load bundle 'other-bundle' alongside".to_owned()
        }
    );
    let selection = PatchSelection {
        enable: ["needs-other-bundle".to_string()].into(),
        ..Default::default()
    };
    let error = engine::validate_patches(&patches, &selection, Some("com.example.test"), None)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "missing bundle: patch runtime-test-bundle/app.reseam.test.needsOtherBundle depends on other-bundle/app.reseam.other.helper; load bundle 'other-bundle' alongside"
    );
}

#[test]
fn universal_patches_wait_to_be_selected() {
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .expect("open runtime bundle")
        .load()
        .expect("load runtime bundle");
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let universal = patches
        .iter()
        .find(|patch| patch.reference() == "runtime-test-bundle/app.reseam.test.universalMarker")
        .expect("the fixture declares a universal patch");
    assert!(universal.spec().compatibility.is_universal());
    assert!(
        !universal.spec().enabled_by_default,
        "a patch that works with any app is opt-in"
    );

    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let selection = PatchSelection {
        enable: ["universal-marker".to_string()].into(),
        ..Default::default()
    };
    let results =
        engine::apply_patches(&mut ctx, &patches, &selection, |_| {}).expect("apply bundle");
    assert_eq!(
        results
            .iter()
            .find(|result| result.patch == universal.reference())
            .map(|result| &result.status),
        Some(&PatchStatus::Applied),
        "selecting it explicitly still runs it"
    );
    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/universal-marker.txt")
            .unwrap(),
        Some(b"universal".to_vec())
    );
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

/// `skipWhen` guards the call on every path into it, and a false condition keeps it.
#[test]
fn skip_when_guards_a_call_on_every_path_into_it() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/SkipHost;";
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let mark = dex
        .intern_method("Lcom/example/Observer;", "mark", "(I)V")
        .unwrap();
    let call = |reg: u8| InvokeStatic {
        method: mark,
        args: [reg].into_iter().collect(),
    };
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "straight",
        "()V",
        (1, 0),
        vec![
            Const4 { dest: 0, value: 1 },
            call(0),
            Const4 { dest: 0, value: 2 },
            call(0),
            Const4 { dest: 0, value: 3 },
            call(0),
            ReturnVoid,
        ],
    );
    // A zero argument branches straight to the guarded call; the other path marks 4 first.
    for name in ["joined", "joinedKept"] {
        add_static_method(
            &mut dex,
            class,
            OWNER,
            name,
            "(I)V",
            (2, 1),
            vec![
                Const4 { dest: 0, value: 1 },
                call(0),
                Const4 { dest: 0, value: 2 },
                IfEqz { a: 1, offset: 6 },
                Const4 { dest: 0, value: 4 },
                call(0),
                call(0),
                Const4 { dest: 0, value: 3 },
                call(0),
                ReturnVoid,
            ],
        );
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "skip-when",
        "skipWhenCall",
    );

    let straight = patched_code(&apk, OWNER, "straight");
    assert_eq!(hook_calls(&straight, &[]), (vec![vec![1], vec![3]], None));
    let joined = patched_code(&apk, OWNER, "joined");
    assert_eq!(hook_calls(&joined, &[0]), (vec![vec![1], vec![3]], None));
    assert_eq!(
        hook_calls(&joined, &[1]),
        (vec![vec![1], vec![4], vec![3]], None)
    );
    // The branch into the guarded call must reach the guard, not jump past it.
    let kept = patched_code(&apk, OWNER, "joinedKept");
    assert_eq!(
        hook_calls(&kept, &[0]),
        (vec![vec![1], vec![2], vec![3]], None)
    );
    assert_eq!(
        hook_calls(&kept, &[1]),
        (vec![vec![1], vec![4], vec![4], vec![3]], None)
    );
}

/// A replaced body whose branches all return carries no dead jump past its end,
/// and one that falls off its end is refused at patch time.
#[test]
fn a_replaced_body_ends_on_every_path_or_fails() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/AnchorHost;";
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "replaced",
        "()V",
        (1, 0),
        vec![ReturnVoid],
    );
    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "replace-both-return",
        "replaceBothReturn",
    );
    let replaced = patched_code(&apk, OWNER, "replaced");
    let end = replaced
        .instructions
        .iter()
        .map(|i| i.code_units() as i32)
        .sum::<i32>();
    let mut addr = 0;
    for insn in &replaced.instructions {
        assert!(
            !matches!(insn, Goto { .. }),
            "no dead goto: {:?}",
            replaced.instructions
        );
        if let IfEqz { offset, .. } = insn {
            assert!(
                addr + (*offset as i32) < end,
                "branch past the end: {:?}",
                replaced.instructions
            );
        }
        addr += insn.code_units() as i32;
    }

    let message = kotlin_patch_failure("replace-falls-through", "replaceFallsThrough");
    assert!(
        message.contains("falls off the end of the method"),
        "got: {message}"
    );
}

/// An app holding `AnchorHost.twice` and `AnchorHost.replaced`, each `const/16 v0, 1; return-void`,
/// and `com.other.Peer.run`, an empty method in another package.
fn anchor_host_apk() -> (tempfile::TempDir, ApkFile) {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/AnchorHost;";
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    for name in ["twice", "replaced"] {
        add_static_method(
            &mut dex,
            class,
            OWNER,
            name,
            "()V",
            (1, 0),
            vec![Const16 { dest: 0, value: 1 }, ReturnVoid],
        );
    }
    const PEER: &str = "Lcom/other/Peer;";
    let peer = dex
        .create_class(PEER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    add_static_method(&mut dex, peer, PEER, "run", "()V", (1, 0), vec![ReturnVoid]);
    open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex])
}

/// Runs one fixture patch against [`anchor_host_apk`] and returns why it failed.
fn kotlin_patch_failure(patch: &str, id: &str) -> String {
    let (_apk_dir, mut apk) = anchor_host_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let selection = PatchSelection {
        enable: [patch.to_string()].into(),
        ..Default::default()
    };
    let results = engine::apply_patches(
        &mut PatchContext::new(&mut apk),
        &patches,
        &selection,
        |_| {},
    )
    .expect("the run itself succeeds");
    let result = results
        .iter()
        .find(|r| r.patch.ends_with(id))
        .unwrap_or_else(|| panic!("{id} not in results"));
    match &result.status {
        PatchStatus::Failed { reason } => reason.clone(),
        other => panic!("{id} should fail, got {other:?}"),
    }
}

/// `setStatic` writes the field.
#[test]
fn set_static_writes_the_field() {
    use reseam_apk::reseam_dex::Instruction::*;

    let (_apk_dir, mut apk) = anchor_host_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "set-static",
        "setStaticField",
    );
    let code = patched_code(&apk, "Lcom/example/AnchorHost;", "twice");
    assert!(
        code.instructions
            .iter()
            .any(|insn| matches!(insn, Sput { .. })),
        "{:?}",
        code.instructions
    );
}

/// Assigning to a value read from a field would overwrite only a copy, so it
/// fails the patch and names the write that was meant.
#[test]
fn assigning_a_field_read_fails_the_patch() {
    let message = kotlin_patch_failure("assign-field-read", "assignFieldRead");
    assert!(
        message.contains("Lcom/example/AnchorHost;->value: it is a copy")
            && message.contains("setStatic(field, value)"),
        "got: {message}"
    );
}

/// Code after a return never runs, so hooking it fails the patch.
#[test]
fn hooking_after_a_return_fails_the_patch() {
    let message = kotlin_patch_failure("after-return", "afterReturn");
    assert!(
        message.contains("nothing runs after the RETURN_VOID"),
        "got: {message}"
    );
}

/// Code that names a member its class cannot reach passes verification and
/// throws `IllegalAccessError` in the app, so the patch fails instead.
#[test]
fn reaching_another_class_private_field_fails_the_patch() {
    let message = kotlin_patch_failure("private-field-access", "privateFieldAccess");
    assert!(
        message.contains(
            "Lcom/example/AnchorHost;->secret is private to com.example.AnchorHost, so com.other.Peer cannot reach it"
        ),
        "got: {message}"
    );
}

/// A call to an extension method the bundle does not define fails the patch,
/// naming what the class does declare, instead of linking a dangling reference
/// that the verifier rejects at class load.
#[test]
fn a_call_to_an_undefined_extension_method_fails_the_patch() {
    let message = kotlin_patch_failure("ext-method-missing", "extMethodMissing");
    assert!(
        message.contains(
            "Lapp/reseam/test/GhostExtension;->run(Z)Z is not in the app or any extension"
        ),
        "got: {message}"
    );
}

/// Passing the wrong number of values to a call fails the patch instead of
/// emitting an invoke the verifier rejects.
#[test]
fn a_call_with_the_wrong_argument_count_fails_the_patch() {
    let message = kotlin_patch_failure("call-arity", "callArity");
    assert!(
        message.contains(
            "valueOf(I)Ljava/lang/String; takes 1 argument(s), but 0 value(s) were passed"
        ),
        "got: {message}"
    );
}

// Follow the fixture's register values through the emitted DEX. Distinct words
// make lost arguments, overlapping scratch spans, and broken wide moves visible.
fn hook_calls(
    code: &reseam_apk::reseam_dex::CodeItem,
    incoming: &[i64],
) -> (Vec<Vec<i64>>, Option<i64>) {
    use reseam_apk::reseam_dex::Instruction::*;
    let mut registers = vec![-1; code.registers_size as usize];
    registers[(code.registers_size - code.ins_size) as usize..].copy_from_slice(incoming);
    let mut calls = Vec::new();
    let offsets: Vec<u32> = code
        .instructions
        .iter()
        .scan(0, |addr, insn| {
            let current = *addr;
            *addr += insn.code_units() as u32;
            Some(current)
        })
        .collect();
    let mut pc = 0;
    for _ in 0..code.instructions.len() * 2 {
        let insn = &code.instructions[pc];
        let movement = match *insn {
            Move { dest, src } | MoveObject { dest, src } => Some((dest as usize, src as usize, 1)),
            MoveFrom16 { dest, src } | MoveObjectFrom16 { dest, src } => {
                Some((dest as usize, src as usize, 1))
            }
            Move16 { dest, src } | MoveObject16 { dest, src } => {
                Some((dest as usize, src as usize, 1))
            }
            MoveWide { dest, src } => Some((dest as usize, src as usize, 2)),
            MoveWideFrom16 { dest, src } => Some((dest as usize, src as usize, 2)),
            MoveWide16 { dest, src } => Some((dest as usize, src as usize, 2)),
            _ => None,
        };
        let wide_constant = match insn {
            ConstWide16 { dest, value } => Some((*dest, i64::from(*value))),
            ConstWide32 { dest, value } => Some((*dest, i64::from(*value))),
            ConstWide { dest, value } => Some((*dest, *value)),
            ConstWideHigh16 { dest, value } => Some((*dest, i64::from(*value) << 48)),
            _ => None,
        };
        if let Some((dest, value)) = wide_constant {
            registers[dest as usize] = i64::from(value as u32);
            registers[dest as usize + 1] = i64::from((value >> 32) as u32);
        } else if let Some((dest, src, width)) = movement {
            registers.copy_within(src..src + width, dest);
        } else {
            match insn {
                Nop => {}
                Const4 { dest, value } => registers[*dest as usize] = *value as i64,
                Const16 { dest, value } => registers[*dest as usize] = *value as i64,
                Const { dest, value } => registers[*dest as usize] = *value as i64,
                InvokeStatic { args, .. } => {
                    calls.push(args.iter().map(|r| registers[*r as usize]).collect())
                }
                InvokeStaticRange {
                    first_reg, count, ..
                } => calls.push(
                    registers[*first_reg as usize..*first_reg as usize + *count as usize].to_vec(),
                ),
                IfEqz { a, offset } if registers[*a as usize] == 0 => {
                    let target = (offsets[pc] as i32 + *offset as i32) as u32;
                    pc = (0..code.instructions.len())
                        .find(|i| offsets[*i] == target)
                        .expect("branch target");
                    continue;
                }
                IfEqz { .. } => {}
                Return { src } | ReturnObject { src } => {
                    return (calls, Some(registers[*src as usize]));
                }
                ReturnWide { src } => {
                    return (
                        calls,
                        Some(registers[*src as usize] | (registers[*src as usize + 1] << 32)),
                    );
                }
                IfNez { a, offset } if registers[*a as usize] != 0 => {
                    let target = (offsets[pc] as i32 + *offset as i32) as u32;
                    pc = offsets.iter().position(|addr| *addr == target).unwrap();
                    continue;
                }
                IfNez { .. } => {}
                Goto { offset } => {
                    let target = (offsets[pc] as i32 + *offset as i32) as u32;
                    pc = offsets.iter().position(|addr| *addr == target).unwrap();
                    continue;
                }
                ReturnVoid => return (calls, None),
                _ => panic!("unexpected fixture instruction: {insn:?}"),
            }
        }
        pc += 1;
    }
    panic!("fixture did not return");
}

#[test]
fn after_hooks_preserve_entry_arguments_when_parameter_registers_are_reused() {
    use reseam_apk::reseam_dex::{
        self as dex, AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::*,
    };

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let owner = "Lcom/example/HookTarget;";
    let class = dex
        .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let five = dex
        .intern_method("Lcom/example/Observer;", "five", "(IJII)V")
        .unwrap();
    let all = dex
        .intern_method("Lcom/example/Observer;", "all", "(IIIIIIIIIJIII)V")
        .unwrap();
    let mut invoke_growth: Vec<_> = (0..9)
        .map(|dest| Const16 {
            dest,
            value: dest as i16,
        })
        .collect();
    invoke_growth.extend([
        InvokeStatic {
            method: five,
            args: [0, 9, 10, 5, 12].into_iter().collect(),
        },
        InvokeStaticRange {
            method: all,
            first_reg: 0,
            count: 14,
        },
        Return { src: 13 },
    ]);
    for (name, proto, access_flags, ins_size, instructions) in [
        (
            "invokeGrowth",
            "(JIII)I",
            AccessFlags::PUBLIC | AccessFlags::STATIC,
            5,
            invoke_growth,
        ),
        (
            "temporaryReuse",
            "(Z)J",
            AccessFlags::PUBLIC | AccessFlags::STATIC,
            1,
            vec![ConstWide16 { dest: 0, value: 7 }, ReturnWide { src: 0 }],
        ),
        (
            "getFeatureSwitchValue",
            "(Ljava/lang/String;JDLjava/lang/String;)Ljava/lang/Object;",
            AccessFlags::PUBLIC | AccessFlags::STATIC,
            6,
            vec![
                IfEqz { a: 10, offset: 5 },
                Const16 { dest: 5, value: 0 },
                ReturnObject { src: 5 },
                ReturnObject { src: 5 },
            ],
        ),
        (
            "receiver",
            "()V",
            AccessFlags::PUBLIC,
            1,
            vec![Const16 { dest: 5, value: 0 }, ReturnVoid],
        ),
        (
            "resultOnly",
            "(I)I",
            AccessFlags::PUBLIC | AccessFlags::STATIC,
            1,
            vec![Return { src: 5 }],
        ),
    ] {
        let method = dex.intern_method(owner, name, proto).unwrap();
        let encoded = EncodedMethod {
            method,
            access_flags,
            code: Some(CodeItem {
                registers_size: if name == "invokeGrowth" {
                    14
                } else {
                    5 + ins_size
                },
                ins_size,
                outs_size: 0,
                debug_info: None,
                instructions,
                tries: vec![],
                catch_handlers: vec![],
            }),
        };
        let class = dex.class_mut(class).unwrap();
        if access_flags.contains(AccessFlags::STATIC) {
            class.add_direct_method(encoded);
        } else {
            class.add_virtual_method(encoded);
        }
    }
    let init = dex.intern_method(owner, "<init>", "()V").unwrap();
    let object_init = dex
        .intern_method("Ljava/lang/Object;", "<init>", "()V")
        .unwrap();
    dex.class_mut(class)
        .unwrap()
        .add_direct_method(EncodedMethod {
            method: init,
            access_flags: AccessFlags::PUBLIC | AccessFlags::CONSTRUCTOR,
            code: Some(CodeItem {
                registers_size: 1,
                ins_size: 1,
                outs_size: 1,
                debug_info: None,
                instructions: vec![
                    InvokeDirect {
                        method: object_init,
                        args: [0].into_iter().collect(),
                    },
                    ReturnVoid,
                ],
                tries: vec![],
                catch_handlers: vec![],
            }),
        });
    // Round trips through the APK before and after patching to exercise lazy decoding and writing.
    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .unwrap()
        .load()
        .unwrap();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let results = engine::apply_patches(
        &mut PatchContext::new(&mut apk),
        &patches,
        &PatchSelection {
            enable: ["after-entry-values".to_string()].into(),
            ..Default::default()
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(
        results
            .iter()
            .find(|r| r.patch == "runtime-test-bundle/app.reseam.test.afterEntryValues")
            .unwrap()
            .status,
        PatchStatus::Applied
    );
    let mut dex = dex::parse(
        &dex::write(apk.dex().dex(0).unwrap()).unwrap(),
        ParseOptions::default(),
    )
    .unwrap();
    dex.resolve_all_class_data().unwrap();
    let class = dex
        .resident_class(dex.find_class_index(owner).unwrap())
        .unwrap();
    let data = class.class_data.as_ref().unwrap();
    for method in data.direct_methods.iter().chain(&data.virtual_methods) {
        let name = dex.string(dex.method_id(method.method).name);
        let code = method.code().unwrap();
        match name.as_ref() {
            "<init>" => {}
            "invokeGrowth" => {
                assert_eq!(
                    hook_calls(code, &[101, 102, 103, 104, 105]),
                    (
                        vec![
                            vec![0, 101, 102, 5, 104],
                            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 101, 102, 103, 104, 105],
                            vec![105]
                        ],
                        Some(105),
                    )
                );
            }
            "getFeatureSwitchValue" => {
                for last in [0, 106] {
                    let result = if last == 0 { 101 } else { 0 };
                    assert_eq!(
                        hook_calls(code, &[101, 102, 103, 104, 105, last]),
                        (
                            vec![vec![42, 101, 102, 103, 104, 105, last, 101, result]],
                            Some(result)
                        )
                    );
                }
            }
            "receiver" => assert_eq!(hook_calls(code, &[201]), (vec![vec![201]], None)),
            "temporaryReuse" => {
                assert!(
                    code.registers_size <= 9,
                    "temporaries must follow peak liveness, got {} registers",
                    code.registers_size
                );
                let mut expected = Vec::new();
                for value in 0..32 {
                    expected.push(vec![value, 0]);
                    expected.push(vec![value]);
                }
                expected.push(vec![0x9abcdef0, 0x12345678]);
                for condition in [0, 1] {
                    assert_eq!(
                        hook_calls(code, &[condition]),
                        (expected.clone(), Some(0x123456789abcdef0))
                    );
                }
            }
            "resultOnly" => {
                assert_eq!(
                    code.registers_size, 6,
                    "no entry reads need no saved locals"
                );
                assert_eq!(hook_calls(code, &[301]), (vec![], Some(42)));
            }
            other => panic!("unexpected method {other}"),
        }
    }
}

#[test]
fn replace_strings_containing_rewrites_only_what_transform_returns() {
    use reseam_apk::reseam_dex::{
        self as dex, AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::*,
    };

    const URI: &str = "content://com.google.android.gsf.gservices/prefix";
    const REWRITTEN: &str = "content://app.reseam.gsf.gservices/prefix";
    const ACTION: &str = "com.google.android.gsf.action.SYNC";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let owner = "Lcom/example/StringHolder;";
    let class = dex
        .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    // The URI sits in two methods, so a count of 2 shows the distinct original
    // was routed through replaceAllStrings once rather than once per hit.
    for (name, constants) in [("uris", &[URI, ACTION][..]), ("more", &[URI][..])] {
        let mut instructions: Vec<_> = constants
            .iter()
            .map(|s| ConstString {
                dest: 0,
                string: dex.intern_string(s),
            })
            .collect();
        instructions.push(ReturnVoid);
        let method = dex.intern_method(owner, name, "()V").unwrap();
        dex.class_mut(class)
            .unwrap()
            .add_direct_method(EncodedMethod {
                method,
                access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
                code: Some(CodeItem {
                    registers_size: 1,
                    ins_size: 0,
                    outs_size: 0,
                    debug_info: None,
                    instructions,
                    tries: vec![],
                    catch_handlers: vec![],
                }),
            });
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .unwrap()
        .load()
        .unwrap();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let results = engine::apply_patches(
        &mut PatchContext::new(&mut apk),
        &patches,
        &PatchSelection {
            enable: ["embedded-strings".to_string()].into(),
            ..Default::default()
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(
        results
            .iter()
            .find(|r| r.patch == "runtime-test-bundle/app.reseam.test.embeddedStrings")
            .unwrap()
            .status,
        PatchStatus::Applied
    );
    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/embedded-strings.txt")
            .unwrap(),
        Some(b"2".to_vec())
    );

    let mut dex = dex::parse(
        &dex::write(apk.dex().dex(0).unwrap()).unwrap(),
        ParseOptions::default(),
    )
    .unwrap();
    dex.resolve_all_class_data().unwrap();
    let class = dex
        .resident_class(dex.find_class_index(owner).unwrap())
        .unwrap();
    let mut constants: Vec<(String, Vec<String>)> = class
        .class_data
        .as_ref()
        .unwrap()
        .direct_methods
        .iter()
        .map(|method| {
            (
                dex.string(dex.method_id(method.method).name).into_owned(),
                method
                    .code()
                    .unwrap()
                    .instructions
                    .iter()
                    .filter_map(|insn| match insn {
                        ConstString { string, .. } => Some(dex.string(*string).into_owned()),
                        _ => None,
                    })
                    .collect(),
            )
        })
        .collect();
    constants.sort();
    assert_eq!(
        constants,
        [
            ("more".to_owned(), vec![REWRITTEN.to_owned()]),
            (
                "uris".to_owned(),
                vec![REWRITTEN.to_owned(), ACTION.to_owned()]
            ),
        ]
    );
}

#[test]
fn same_named_patches_keep_independent_identity_options_dependencies_and_settings() {
    use reseam_patcher::engine::{PatchIndex, validate_patches};
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .unwrap()
        .load()
        .unwrap();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let first = "runtime-test-bundle/app.reseam.test.firstAds";
    let second = "runtime-test-bundle/app.reseam.test.secondAds";
    let other = "runtime-test-bundle/app.reseam.test.otherAds";
    let index = PatchIndex::new(&patches).unwrap();
    for id in [first, second, other] {
        let patch = patches[index.resolve(id, None).unwrap()];
        assert_eq!(patch.reference(), id);
        assert_eq!(patch.spec().name, "Hide Ads");
    }
    assert!(
        patches[index.resolve(second, None).unwrap()]
            .spec()
            .dependencies
            .iter()
            .any(|id| id == first)
    );
    let error = index
        .resolve("Hide Ads", Some("com.example.test"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("ambiguous") && error.contains(first) && error.contains(second));
    assert!(!error.contains(other));

    let selection = PatchSelection {
        enable: ["Hide Ads".to_owned()].into(),
        ..Default::default()
    };
    let results = validate_patches(&patches, &selection, Some("com.example.other"), None).unwrap();
    let applied: Vec<_> = results
        .iter()
        .filter(|r| r.status == PatchStatus::Applied)
        .map(|r| r.patch.as_str())
        .collect();
    assert_eq!(applied, [other]);

    let (_apk_dir, mut apk) = open_split_test_apk();
    let mut first_options = std::collections::HashMap::new();
    first_options.insert("marker".to_owned(), OptionValue::Text("one".into()));
    let mut second_options = std::collections::HashMap::new();
    second_options.insert("marker".to_owned(), OptionValue::Text("two".into()));
    let selection = PatchSelection {
        enable: [second.to_owned()].into(),
        options: [
            (first.to_owned(), first_options),
            (second.to_owned(), second_options),
        ]
        .into(),
        ..Default::default()
    };
    let results = engine::apply_patches(
        &mut PatchContext::new(&mut apk),
        &patches,
        &selection,
        |_| {},
    )
    .unwrap();
    let applied: Vec<_> = results
        .iter()
        .filter(|r| r.status == PatchStatus::Applied)
        .map(|r| r.patch.as_str())
        .collect();
    assert!(
        applied.iter().position(|id| *id == first).unwrap()
            < applied.iter().position(|id| *id == second).unwrap()
    );
    for (path, expected) in [
        ("assets/first-ads.txt", b"one"),
        ("assets/second-ads.txt", b"two"),
    ] {
        assert_eq!(
            apk.component_mut(0)
                .unwrap()
                .read_entry(path)
                .unwrap()
                .unwrap(),
            expected
        );
    }
    let schema = apk
        .component_mut(0)
        .unwrap()
        .read_entry("assets/reseam/settings.json")
        .unwrap()
        .unwrap();
    let schema: serde_json::Value = serde_json::from_slice(&schema).unwrap();
    let sections = schema["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["title"], "First");
    assert_eq!(sections[1]["title"], "Second");

    let selection = PatchSelection {
        disable: [first.to_owned()].into(),
        ..selection
    };
    let results = validate_patches(
        &patches,
        &PatchSelection {
            options: Default::default(),
            ..selection
        },
        Some("com.example.test"),
        None,
    )
    .unwrap();
    assert!(
        results
            .iter()
            .filter(|r| r.patch == first || r.patch == second)
            .all(|r| matches!(r.status, PatchStatus::Skipped { .. }))
    );

    let conflict = PatchSelection {
        enable: ["runtime-test-bundle/app.reseam.test.runtimeApi".to_owned()].into(),
        disable: ["runtime-api".to_owned()].into(),
        ..Default::default()
    };
    assert!(
        validate_patches(&patches, &conflict, Some("com.example.test"), None)
            .unwrap_err()
            .to_string()
            .contains("both selected and disabled")
    );
}

/// A class with one method per entry, each holding the given constants.
fn dex_with_string_methods(
    owner: &str,
    superclass: &str,
    methods: &[(&str, &[&str])],
) -> reseam_apk::reseam_dex::DexFile {
    use reseam_apk::reseam_dex::{
        AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::ConstString,
        Instruction::ReturnVoid,
    };

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(owner, AccessFlags::PUBLIC, Some(superclass))
        .unwrap();
    for (name, constants) in methods {
        let mut instructions: Vec<_> = constants
            .iter()
            .map(|value| ConstString {
                dest: 0,
                string: dex.intern_string(value),
            })
            .collect();
        instructions.push(ReturnVoid);
        let method = dex.intern_method(owner, name, "()V").unwrap();
        dex.class_mut(class)
            .unwrap()
            .add_direct_method(EncodedMethod {
                method,
                access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
                code: Some(CodeItem {
                    registers_size: 1,
                    ins_size: 0,
                    outs_size: 0,
                    debug_info: None,
                    instructions,
                    tries: vec![],
                    catch_handlers: vec![],
                }),
            });
    }
    dex
}

/// Applies the one patch `name` selects and fails with its status when it did not.
fn run_one_patch(ctx: &mut PatchContext<'_>, patches: &[&dyn Patch], name: &str, id: &str) {
    let results = engine::apply_patches(
        ctx,
        patches,
        &PatchSelection {
            enable: [name.to_string()].into(),
            ..Default::default()
        },
        |_| {},
    )
    .unwrap();
    let applied = results
        .iter()
        .find(|result| result.patch == format!("runtime-test-bundle/app.reseam.test.{id}"));
    assert!(
        matches!(
            applied.map(|result| &result.status),
            Some(PatchStatus::Applied)
        ),
        "{name} did not apply: {:?}",
        results
            .iter()
            .map(|result| (&result.patch, &result.status))
            .collect::<Vec<_>>()
    );
}

fn loaded_test_bundle() -> reseam_patcher::bundle::PatchBundle {
    BundleArchive::open(&write_bundle_reseam().path)
        .unwrap()
        .load()
        .unwrap()
}

#[test]
fn app_entry_unseals_the_final_on_create_it_overrides() {
    use reseam_apk::reseam_dex::{
        self as dex, AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::ReturnVoid,
    };

    const BASE: &str = "Lcom/example/AppBase;";
    const APP: &str = "Lcom/example/App;";

    // The base class sits in its own DEX. A superclass walk that stopped at the file boundary
    // would miss the final onCreate and emit an app ART refuses to load.
    let mut base_dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let base_class = base_dex
        .create_class(BASE, AccessFlags::PUBLIC, Some("Landroid/app/Application;"))
        .unwrap();
    let on_create = base_dex.intern_method(BASE, "onCreate", "()V").unwrap();
    base_dex
        .class_mut(base_class)
        .unwrap()
        .add_virtual_method(EncodedMethod {
            method: on_create,
            access_flags: AccessFlags::PUBLIC | AccessFlags::FINAL,
            code: Some(CodeItem {
                registers_size: 1,
                ins_size: 1,
                outs_size: 0,
                debug_info: None,
                instructions: vec![ReturnVoid],
                tries: vec![],
                catch_handlers: vec![],
            }),
        });

    let mut app_dex = DexFile::new(empty_dex_header(DexVersion::V035));
    app_dex
        .create_class(APP, AccessFlags::PUBLIC, Some(BASE))
        .unwrap();

    let (_apk_dir, mut apk) = open_split_test_apk_with(
        manifest_with_application("com.example.App"),
        &[base_dex, app_dex],
    );
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "app-entry-hook",
        "appEntryHook",
    );

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/app-entry.txt")
            .unwrap(),
        Some(APP.as_bytes().to_vec()),
        "the entry point must stay on the class the manifest names"
    );

    let written = dex::parse(
        &dex::write(apk.dex().dex(1).unwrap()).unwrap(),
        ParseOptions::default(),
    )
    .unwrap();
    assert!(
        method_flags(&written, APP, "onCreate").is_some(),
        "appEntry added no onCreate to {APP}"
    );
    let base = dex::parse(
        &dex::write(apk.dex().dex(0).unwrap()).unwrap(),
        ParseOptions::default(),
    )
    .unwrap();
    assert!(
        !method_flags(&base, BASE, "onCreate")
            .expect("base onCreate")
            .contains(AccessFlags::FINAL),
        "the inherited onCreate is still final, so the override is a LinkageError"
    );
}

/// The access flags of the named method, whichever list it is declared in.
fn method_flags(
    dex: &reseam_apk::reseam_dex::DexFile,
    owner: &str,
    name: &str,
) -> Option<reseam_apk::reseam_dex::AccessFlags> {
    let mut dex = dex.clone();
    dex.resolve_all_class_data().unwrap();
    let class = dex.resident_class(dex.find_class_index(owner)?)?;
    let data = class.class_data.as_ref()?;
    data.direct_methods
        .iter()
        .chain(data.virtual_methods.iter())
        .find(|method| dex.string(dex.method_id(method.method).name) == name)
        .map(|method| method.access_flags)
}

#[test]
fn queries_skip_the_classes_a_bundle_extension_defines() {
    use reseam_patcher::context::ExtensionSet;

    const APP_STATE: &str = "Lcom/example/VideoState;";
    const EXT_STATE: &str = "Lapp/reseam/test/ext/VideoState;";
    const STATES: &[&str] = &["NEW", "PLAYING"];

    let extension_dir = tempfile::tempdir().expect("tempdir failed");
    let extension_path = extension_dir.path().join("ext.dex");
    let extension = dex_with_string_methods(EXT_STATE, "Ljava/lang/Object;", &[("names", STATES)]);
    fs::write(
        &extension_path,
        reseam_apk::reseam_dex::write(&extension).unwrap(),
    )
    .unwrap();

    let (_apk_dir, mut apk) = open_split_test_apk_with(
        manifest_bytes("1.0-base", None),
        &[dex_with_string_methods(
            APP_STATE,
            "Ljava/lang/Object;",
            &[("names", STATES)],
        )],
    );
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    let mut ctx = PatchContext::new(&mut apk);
    ctx.set_extensions(ExtensionSet::load(&[extension_path]).unwrap());
    run_one_patch(&mut ctx, &patches, "extension-shadow", "extensionShadow");

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/extension-shadow.txt")
            .unwrap(),
        Some(format!("{APP_STATE}|{EXT_STATE}").into_bytes()),
        "a plain query must find the app's class; includeExtensions() widens it to the bundle's"
    );
}

#[test]
fn flags_require_every_bit_of_the_mask() {
    use reseam_apk::reseam_dex::{
        AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::ReturnVoid,
    };

    const OWNER: &str = "Lcom/example/FlagHolder;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    // Both are final; only one is public, so an any-bit flags() would match them both.
    for (name, visibility) in [
        ("hidden", AccessFlags::PRIVATE),
        ("shown", AccessFlags::PUBLIC),
    ] {
        let method = dex.intern_method(OWNER, name, "()V").unwrap();
        dex.class_mut(class)
            .unwrap()
            .add_virtual_method(EncodedMethod {
                method,
                access_flags: visibility | AccessFlags::FINAL,
                code: Some(CodeItem {
                    registers_size: 1,
                    ins_size: 1,
                    outs_size: 0,
                    debug_info: None,
                    instructions: vec![ReturnVoid],
                    tries: vec![],
                    catch_handlers: vec![],
                }),
            });
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "all-access-flags",
        "allAccessFlags",
    );

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/all-access-flags.txt")
            .unwrap(),
        Some(b"shown".to_vec())
    );
}

#[test]
fn a_new_attribute_binds_the_id_the_inflater_resolves_it_by() {
    const TARGET_PACKAGE: u32 = 0x0101_0021;
    const TARGET_CLASS: u32 = 0x0101_002f;

    let (_apk_dir, mut apk) = open_split_test_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "xml-attribute-binding",
        "xmlAttributeBinding",
    );

    let manifest = apk.base().manifest();
    let intent = manifest
        .find_element("intent")
        .expect("the patch appended an <intent> element");
    let value = |res_id| {
        manifest
            .attribute(intent, res_id)
            .and_then(|attr| manifest.attribute_string(attr))
            .map(|value| value.into_owned())
    };
    assert_eq!(value(TARGET_PACKAGE).as_deref(), Some("com.example.target"));
    assert_eq!(
        value(TARGET_CLASS).as_deref(),
        Some("com.example.target.Settings")
    );
    let ids: Vec<_> = manifest
        .attributes(intent)
        .iter()
        .map(|attr| manifest.resource_id_for(attr.name))
        .collect();
    assert_eq!(ids, [Some(TARGET_PACKAGE), Some(TARGET_CLASS)]);
}

#[test]
fn a_typed_file_resource_a_style_and_an_array_land_where_the_loader_reads_them() {
    const WINDOW_BACKGROUND: u32 = 0x0101_0054;
    const TEXT_COLOR: u32 = 0x0101_0098;

    let (_apk_dir, mut apk) = open_split_test_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "resource-entries",
        "resourceEntries",
    );

    let base = apk.component_mut(0).unwrap();
    assert_eq!(
        base.read_entry("assets/resource-entries.txt").unwrap(),
        Some(format!("{LAYOUT_PATH}|5,10").into_bytes()),
        "the layout resolved to its obfuscated path and the array read back as text"
    );
    assert!(base.read_entry("res/reseam_icon.png").unwrap().is_some());
    let pulse_bytes = base
        .read_entry("res/drawable/reseam_pulse.xml")
        .unwrap()
        .unwrap();
    let pulse_vector_bytes = base
        .read_entry("res/drawable/$reseam_pulse__0.xml")
        .unwrap()
        .unwrap();
    let table = base.resources().unwrap().unwrap();

    // A file resource is a string entry of the typed kind, which is what
    // Resources.getDrawable and layout inflation follow.
    let icon = table.find_resource_id("drawable", "reseam_icon").unwrap();
    assert_eq!(
        table.file_paths("drawable", "reseam_icon").unwrap(),
        ["res/reseam_icon.png"]
    );
    assert_eq!(
        table.file_paths("mipmap", "reseam_launcher").unwrap(),
        ["res/reseam_launcher.png"]
    );

    // Inline <aapt:attr> resources are files of their own that the parent references.
    let vector = table
        .find_resource_id("drawable", "$reseam_pulse__0")
        .unwrap();
    let animator = table
        .find_resource_id("drawable", "$reseam_pulse__1")
        .unwrap();
    assert_eq!(
        table.file_paths("drawable", "$reseam_pulse__1").unwrap(),
        ["res/drawable/$reseam_pulse__1.xml"]
    );
    let pulse = reseam_apk::AxmlDocument::parse(&pulse_bytes).unwrap();
    let root = pulse.root().unwrap();
    assert_eq!(
        pulse.attribute_named(root, "drawable").unwrap().value,
        ResValue::reference(vector)
    );
    let target = (0..pulse.elements.len())
        .find(|&i| pulse.element_name(i).as_deref() == Some("target"))
        .unwrap();
    assert_eq!(
        pulse.attribute_named(target, "animation").unwrap().value,
        ResValue::reference(animator)
    );
    assert!(reseam_apk::AxmlDocument::parse(&pulse_vector_bytes).is_ok());
    assert_eq!(
        table
            .find_resource_id("mipmap", "launcher")
            .map(|id| id & 0xFFFF),
        Some(0),
        "the density configuration keeps the entry it already had"
    );

    let items = |name: &str| {
        table
            .complex_entries("style", name)
            .unwrap_or_else(|| panic!("style/{name} is not a complex entry"))
            .into_iter()
            .map(|entry| (entry.name, entry.value))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        items("Theme.Test"),
        [
            (WINDOW_BACKGROUND, ResValue::reference(icon)),
            (
                TEXT_COLOR,
                ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff10_2030)
            ),
        ],
        "the existing item was replaced, the new one added, and both kept in id order"
    );
    let app_attr = table
        .find_resource_id("attr", "layout_constraintRight_toLeftOf")
        .unwrap();
    assert_eq!(
        items("Theme.Reseam").first().map(|item| item.0),
        Some(app_attr)
    );
    assert_eq!(
        table.array("double_tap_lengths").unwrap(),
        [
            "5",
            "10",
            "15",
            "20",
            &format!("@0x{icon:08x}"),
            "true",
            "@string/missing"
        ],
        "the element count grew and every element kept a text form"
    );
    assert!(
        table
            .complex_entries("array", "double_tap_lengths")
            .unwrap()
            .iter()
            .all(|entry| entry.value.kind == ResValue::STRING)
    );
    let bytes = base.read_entry("res/split.xml").unwrap().unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert_eq!(
        doc.element_name(doc.root().unwrap()).as_deref(),
        Some("base")
    );
    assert!(doc.find_element("split").is_none());
    let split = apk.component_mut(1).unwrap();
    let bytes = split.read_entry("res/split.xml").unwrap().unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert!(doc.find_element("split").is_some());
}

#[test]
fn an_adopted_subtree_carries_the_ids_the_inflater_resolves_it_by() {
    const RES_AUTO: &str = "http://schemas.android.com/apk/res-auto";
    const ANDROID_ID: u32 = 0x0101_00d0;
    const ANDROID_SRC: u32 = 0x0101_0119;

    let (_apk_dir, mut apk) = open_split_test_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "xml-graft",
        "xmlGraft",
    );

    let base = apk.component_mut(0).unwrap();
    let layout = base.read_entry(LAYOUT_PATH).unwrap().expect("the layout");
    let layout = reseam_apk::AxmlDocument::parse(&layout).expect("parse the patched layout");
    let table = base.resources().unwrap().unwrap();
    let app_attr = table
        .find_resource_id("attr", "layout_constraintRight_toLeftOf")
        .unwrap();
    let controls = table.find_resource_id("id", "controls").unwrap();
    let fullscreen = table.find_resource_id("id", "fullscreen_button").unwrap();
    let icon = table.find_resource_id("drawable", "reseam_icon").unwrap();

    let container = layout
        .find_element("FrameLayout")
        .expect("the grafted node");
    let res_auto = layout
        .namespace_index(RES_AUTO)
        .expect("the layout declares res-auto");
    let constraint = layout
        .attribute(container, app_attr)
        .expect("the adopted attribute kept an app resource id");
    assert_eq!(constraint.value, ResValue::reference(fullscreen));
    assert_eq!(
        constraint.namespace,
        Some(res_auto),
        "the source prefix was yt, the target declares app, and both mean res-auto"
    );
    assert_eq!(
        layout
            .attribute(container, ANDROID_ID)
            .map(|attr| attr.value.kind),
        Some(ResValue::REFERENCE)
    );
    assert_eq!(
        layout.element_name(container).as_deref(),
        Some("FrameLayout")
    );
    assert!(
        layout.attribute_named(container, "ignore").is_none(),
        "tools attributes are build-time hints the compiler drops"
    );

    let image = (0..layout.elements.len())
        .filter(|&i| layout.element_name(i).as_deref() == Some("ImageView"))
        .collect::<Vec<_>>();
    assert_eq!(image.len(), 2);
    assert_eq!(
        layout
            .attribute(image[1], ANDROID_SRC)
            .map(|attr| attr.value),
        Some(ResValue::reference(icon)),
        "the fragment compiled against the drawable the patch had just registered"
    );
    assert_eq!(
        layout.attribute(image[0], app_attr).map(|attr| attr.value),
        Some(ResValue::reference(controls)),
        "set binds an app attribute on a document that declares res-auto"
    );

    let manifest = apk.base().manifest();
    assert!(
        manifest.namespace_index(RES_AUTO).is_some(),
        "declareNamespace added res-auto to a document that lacked it"
    );
    assert_eq!(
        manifest
            .attribute(manifest.root().unwrap(), app_attr)
            .map(|attr| attr.value),
        Some(ResValue::reference(controls))
    );
}

#[test]
fn a_query_can_require_instruction_order_and_a_predicate_of_its_own() {
    use reseam_apk::reseam_dex::{AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction};

    const OWNER: &str = "Lcom/example/ShapeHolder;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let string = dex.intern_string("shape");
    let number = Instruction::Const { dest: 0, value: 1 };
    let text = Instruction::ConstString { dest: 0, string };
    // alpha and beta hold the same opcodes in the other order, so only their
    // order tells them apart; gamma is the one a length predicate picks.
    for (name, instructions) in [
        (
            "alpha",
            vec![number.clone(), text.clone(), Instruction::ReturnVoid],
        ),
        ("beta", vec![text, number, Instruction::ReturnVoid]),
        ("gamma", vec![Instruction::ReturnVoid]),
    ] {
        let method = dex.intern_method(OWNER, name, "()V").unwrap();
        dex.class_mut(class)
            .unwrap()
            .add_direct_method(EncodedMethod {
                method,
                access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
                code: Some(CodeItem {
                    registers_size: 1,
                    ins_size: 0,
                    outs_size: 0,
                    debug_info: None,
                    instructions,
                    tries: vec![],
                    catch_handlers: vec![],
                }),
            });
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "instruction-shape-queries",
        "instructionShapeQueries",
    );

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/instruction-shape.txt")
            .unwrap(),
        Some(b"alpha|gamma".to_vec()),
    );
}

#[test]
fn a_class_query_can_name_the_source_file_the_compiler_recorded() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile};

    const BINDER: &str = "Lcom/example/Binder;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    for (descriptor, source_file) in [
        (BINDER, "LithoRVSLCBinder.java"),
        ("Lcom/example/Other;", "Other.java"),
    ] {
        let class = dex
            .create_class(descriptor, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
            .unwrap();
        let name = dex.intern_string(source_file);
        dex.class_mut(class).unwrap().source_file = Some(name);
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "source-file-query",
        "sourceFileQuery",
    );

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/source-file.txt")
            .unwrap(),
        Some(BINDER.as_bytes().to_vec()),
    );
}

#[test]
fn a_class_reaches_the_methods_its_base_declares() {
    use reseam_apk::reseam_dex::{
        AccessFlags, CodeItem, DexFile, EncodedMethod, Instruction::InvokeVirtual,
        Instruction::ReturnVoid,
    };

    const FRAGMENT: &str = "Lcom/example/PreferenceFragment;";
    const BASE: &str = "Lcom/example/PreferenceBase;";
    const CALLER: &str = "Lcom/example/Caller;";

    let body = |instructions| {
        Some(CodeItem {
            registers_size: 1,
            ins_size: 0,
            outs_size: 1,
            debug_info: None,
            instructions,
            tries: vec![],
            catch_handlers: vec![],
        })
    };

    // The base sits in another DEX, the way an obfuscated support library does.
    let mut base_dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let base_class = base_dex
        .create_class(BASE, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let declared = base_dex
        .intern_method(BASE, "findPreference", "()V")
        .unwrap();
    base_dex
        .class_mut(base_class)
        .unwrap()
        .add_virtual_method(EncodedMethod {
            method: declared,
            access_flags: AccessFlags::PUBLIC,
            code: body(vec![ReturnVoid]),
        });

    let mut app_dex = DexFile::new(empty_dex_header(DexVersion::V035));
    app_dex
        .create_class(FRAGMENT, AccessFlags::PUBLIC, Some(BASE))
        .unwrap();
    let caller_class = app_dex
        .create_class(CALLER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    // The call site names the subclass, which is the only name the app ever uses.
    let callee = app_dex
        .intern_method(FRAGMENT, "findPreference", "()V")
        .unwrap();
    let run = app_dex.intern_method(CALLER, "run", "()V").unwrap();
    app_dex
        .class_mut(caller_class)
        .unwrap()
        .add_direct_method(EncodedMethod {
            method: run,
            access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
            code: body(vec![
                InvokeVirtual {
                    method: callee,
                    args: [0u8].into_iter().collect(),
                },
                ReturnVoid,
            ]),
        });

    let (_apk_dir, mut apk) =
        open_split_test_apk_with(manifest_bytes("1.0-base", None), &[app_dex, base_dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "inherited-methods",
        "inheritedMethods",
    );

    assert_eq!(
        apk.component_mut(0)
            .unwrap()
            .read_entry("assets/inherited-methods.txt")
            .unwrap(),
        Some(format!("{BASE}|{BASE}").into_bytes()),
        "inClass(inherited) and a call site's callee both resolve to the base declaration",
    );
}

/// Adds a public static method to `class` with `frame` as (registers, ins) and the given body.
fn add_static_method(
    dex: &mut reseam_apk::reseam_dex::DexFile,
    class: usize,
    owner: &str,
    name: &str,
    proto: &str,
    frame: (u16, u16),
    instructions: Vec<reseam_apk::reseam_dex::Instruction>,
) {
    use reseam_apk::reseam_dex::{AccessFlags, CodeItem, EncodedMethod};

    let (registers_size, ins_size) = frame;
    let method = dex.intern_method(owner, name, proto).unwrap();
    dex.class_mut(class)
        .unwrap()
        .add_direct_method(EncodedMethod {
            method,
            access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
            code: Some(CodeItem {
                registers_size,
                ins_size,
                outs_size: 0,
                debug_info: None,
                instructions,
                tries: vec![],
                catch_handlers: vec![],
            }),
        });
}

/// The code of `owner->name` once the patched APK has been written and parsed again.
fn patched_code(apk: &ApkFile, owner: &str, name: &str) -> reseam_apk::reseam_dex::CodeItem {
    use reseam_apk::reseam_dex as dex;

    let mut dex = dex::parse(
        &dex::write(apk.dex().dex(0).unwrap()).unwrap(),
        ParseOptions::default(),
    )
    .unwrap();
    dex.resolve_all_class_data().unwrap();
    let class = dex
        .resident_class(dex.find_class_index(owner).unwrap())
        .unwrap();
    let data = class.class_data.as_ref().unwrap();
    data.direct_methods
        .iter()
        .chain(&data.virtual_methods)
        .find(|method| dex.string(dex.method_id(method.method).name) == name)
        .unwrap_or_else(|| panic!("{owner}->{name} is missing"))
        .code()
        .unwrap()
        .clone()
}

/// The registers each `invoke-static` in the method reads, in method order.
fn static_call_registers(code: &reseam_apk::reseam_dex::CodeItem) -> Vec<Vec<usize>> {
    code.instructions
        .iter()
        .filter_map(|insn| match insn {
            reseam_apk::reseam_dex::Instruction::InvokeStatic { args, .. } => {
                Some(args.iter().map(|r| *r as usize).collect())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn emissions_at_one_point_land_in_order_and_a_replaced_body_loses_it() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/AnchorHost;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    for name in ["twice", "replaced"] {
        add_static_method(
            &mut dex,
            class,
            OWNER,
            name,
            "()V",
            (1, 0),
            vec![
                Const16 { dest: 0, value: 1 },
                Const16 { dest: 0, value: 2 },
                Const16 { dest: 0, value: 3 },
                ReturnVoid,
            ],
        );
    }

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "point-anchors",
        "pointAnchors",
    );

    // Entry code, the block before the instruction, then the after-blocks in
    // the order the patch emitted them, though an entry hook moved the point
    // between the second and third.
    let twice = patched_code(&apk, OWNER, "twice");
    assert_eq!(
        hook_calls(&twice, &[]),
        (vec![vec![40], vec![30], vec![10], vec![20], vec![50]], None)
    );
    let constants: Vec<i16> = twice
        .instructions
        .iter()
        .filter_map(|insn| match insn {
            Const16 { value, .. } => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(constants, [40, 1, 30, 2, 10, 20, 50, 3]);
    assert_eq!(
        patched_code(&apk, OWNER, "replaced").instructions,
        [Const4 { dest: 0, value: 0 }, ReturnVoid],
        "a point in a replaced body fails before emitting anything"
    );
}

#[test]
fn a_reserved_local_carries_a_value_between_blocks() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/LocalHost;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "carry",
        "(I)I",
        (2, 1),
        vec![
            Const16 { dest: 0, value: 7 },
            Const16 { dest: 0, value: 8 },
            Const16 { dest: 0, value: 9 },
            Return { src: 0 },
        ],
    );

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "reserved-locals",
        "reservedLocals",
    );

    let carry = patched_code(&apk, OWNER, "carry");
    // Both initializers precede the entry hook, in reservation order.
    assert_eq!(
        carry.instructions[..2],
        [
            Const4 { dest: 1, value: 0 },
            ConstWide16 { dest: 2, value: 0 }
        ]
    );
    // The pair block needs two registers at once; without the reservation it
    // would take v1 and lose the 77.
    assert_eq!(
        hook_calls(&carry, &[5]),
        (vec![vec![0], vec![1, 2], vec![77], vec![1, 1]], Some(9))
    );
    let reads = static_call_registers(&carry);
    assert_eq!(reads[0], [1]);
    assert_eq!(reads[2], [1]);
    assert_eq!(reads[3], [2, 3]);
}

#[test]
fn an_argument_capture_names_the_register_the_invoke_passes() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/ArgHost;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let greet = dex
        .intern_method(OWNER, "greet", "(Ljava/lang/String;I)V")
        .unwrap();
    let wide2 = dex
        .intern_method("Lcom/example/Observer;", "wide2", "(IJI)V")
        .unwrap();
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "run",
        "(Lcom/example/ArgHost;Ljava/lang/String;)V",
        (4, 2),
        vec![
            Const16 { dest: 0, value: 5 },
            InvokeVirtual {
                method: greet,
                args: [2, 3, 0].into_iter().collect(),
            },
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "wide",
        "(JI)V",
        (5, 3),
        vec![
            Const16 { dest: 0, value: 1 },
            InvokeStatic {
                method: wide2,
                args: [0, 2, 3, 4].into_iter().collect(),
            },
            ReturnVoid,
        ],
    );

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "captured-arguments",
        "capturedArguments",
    );

    // Receiver, string, int: the same registers the virtual invoke passes.
    let run = patched_code(&apk, OWNER, "run");
    assert_eq!(static_call_registers(&run), [vec![2, 3, 0]]);
    let seen = run
        .instructions
        .iter()
        .position(|insn| matches!(insn, InvokeStatic { .. }))
        .unwrap();
    let greet_at = run
        .instructions
        .iter()
        .position(|insn| matches!(insn, InvokeVirtual { .. }))
        .unwrap();
    assert!(seen < greet_at, "the before-block precedes the invoke");
    // The long occupies v2 and v3 and counts as one argument, so argument 2 is v4.
    let wide = patched_code(&apk, OWNER, "wide");
    assert_eq!(
        static_call_registers(&wide),
        [vec![2, 3, 4], vec![0, 2, 3, 4]]
    );
}

#[test]
fn a_writer_is_the_one_instruction_that_defined_an_argument() {
    use reseam_apk::reseam_dex::{AccessFlags, DexFile, Instruction::*};

    const OWNER: &str = "Lcom/example/WriterHost;";

    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let take = dex
        .intern_method("Lcom/example/Observer;", "take", "(I)V")
        .unwrap();
    let call = |register: u8| InvokeStatic {
        method: take,
        args: [register].into_iter().collect(),
    };
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "single",
        "(I)V",
        (3, 1),
        vec![
            Const16 { dest: 0, value: 3 },
            Const16 { dest: 1, value: 4 },
            call(1),
            ReturnVoid,
        ],
    );
    // v1 is written on both arms of the branch before the call reads it.
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "merged",
        "(Z)V",
        (3, 1),
        vec![
            IfEqz { a: 2, offset: 5 },
            Const16 { dest: 1, value: 4 },
            Goto { offset: 3 },
            Const16 { dest: 1, value: 5 },
            call(1),
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "passed",
        "(I)V",
        (1, 1),
        vec![call(0), ReturnVoid],
    );

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "register-writers",
        "registerWriters",
    );

    let report = apk
        .component_mut(0)
        .unwrap()
        .read_entry("assets/register-writers.txt")
        .unwrap()
        .unwrap();
    let report = String::from_utf8(report).unwrap();
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines[0], "1:CONST_16");
    assert!(
        lines[1].contains("merged")
            && lines[1]
                .contains("[4] is written on more than one path in: [1] CONST_16, [3] CONST_16"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].contains("passed")
            && lines[2].contains(
                "[0] is a value the method was passed, not one an instruction of it wrote"
            ),
        "{}",
        lines[2]
    );
    // The writer is a point of its own: code after it runs between the write
    // and the call that reads it.
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "single"), &[6]),
        (vec![vec![99], vec![4]], None)
    );
}

/// Exercises the public SDK against real DEX, including copies, merges, exception
/// edges, entry values, duplicate invokes, range arguments and edits after selection.
#[test]
fn point_queries_and_redirects_share_tracked_control_flow() {
    use reseam_apk::reseam_dex::{AccessFlags, CatchHandler, Instruction::*, TryItem};
    const OWNER: &str = "Lcom/example/PointQueryHost;";
    const OBSERVER: &str = "Lcom/example/PointObserver;";
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let class = dex
        .create_class(OWNER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let observer = dex
        .create_class(OBSERVER, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let take = dex.intern_method(OBSERVER, "take", "(I)V").unwrap();
    let compute = dex.intern_method(OBSERVER, "compute", "(I)I").unwrap();
    let unknown_take = dex.intern_method(OBSERVER, "take", "(I)I").unwrap();
    let take_wide = dex
        .intern_method("Ljava/lang/Object;", "take", "(JI)V")
        .unwrap();
    let call = || InvokeStatic {
        method: take,
        args: [1].into_iter().collect(),
    };
    let computing = || InvokeStatic {
        method: compute,
        args: [1].into_iter().collect(),
    };
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "copied",
        "()V",
        (3, 0),
        vec![
            Const16 { dest: 0, value: 4 },
            Move { dest: 1, src: 0 },
            call(),
            ReturnVoid,
        ],
    );
    for (name, second) in [("same", 4), ("conflict", 5)] {
        add_static_method(
            &mut dex,
            class,
            OWNER,
            name,
            "(Z)V",
            (3, 1),
            vec![
                IfEqz { a: 2, offset: 5 },
                Const16 { dest: 1, value: 4 },
                Goto { offset: 3 },
                Const16 {
                    dest: 1,
                    value: second,
                },
                call(),
                ReturnVoid,
            ],
        );
    }
    let take_long = dex.intern_method(OBSERVER, "take", "(J)V").unwrap();
    for (name, broken) in [("wideCopied", false), ("wideBroken", true)] {
        let mut instructions = vec![
            ConstWide16 { dest: 0, value: 4 },
            MoveWide { dest: 2, src: 0 },
        ];
        if broken {
            instructions.push(Const4 { dest: 3, value: 0 });
        }
        instructions.extend([
            InvokeStatic {
                method: take_long,
                args: [2, 3].into_iter().collect(),
            },
            ReturnVoid,
        ]);
        add_static_method(&mut dex, class, OWNER, name, "()V", (4, 0), instructions);
    }
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "passed",
        "(I)V",
        (2, 1),
        vec![call(), ReturnVoid],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "wide",
        "(Ljava/lang/Object;JI)V",
        (20, 4),
        vec![
            Const16 { dest: 19, value: 4 },
            InvokeVirtualRange {
                method: take_wide,
                first_reg: 16,
                count: 4,
            },
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "duplicates",
        "(I)V",
        (2, 1),
        vec![Const16 { dest: 1, value: 4 }, call(), call(), ReturnVoid],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "overwritten",
        "()V",
        (2, 0),
        vec![
            Const16 { dest: 1, value: 4 },
            call(),
            Const16 { dest: 1, value: 5 },
            call(),
            ReturnVoid,
        ],
    );
    // Unsupported control flow is unknown, not a positive literal match.
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "unknown",
        "()V",
        (2, 0),
        vec![
            Const16 { dest: 1, value: 4 },
            Goto { offset: 100 },
            InvokeStatic {
                method: unknown_take,
                args: [1].into_iter().collect(),
            },
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "exception",
        "()V",
        (2, 0),
        vec![
            Const16 { dest: 1, value: 4 },
            computing(),
            ReturnVoid,
            MoveException { dest: 0 },
            call(),
            ReturnVoid,
        ],
    );
    let exception_id = dex.intern_method(OWNER, "exception", "()V").unwrap();
    let code = dex
        .class_mut(class)
        .unwrap()
        .class_data
        .as_mut()
        .unwrap()
        .direct_methods
        .iter_mut()
        .find(|method| method.method == exception_id)
        .unwrap()
        .code
        .as_mut()
        .unwrap();
    code.tries.push(TryItem {
        start_addr: 2,
        insn_count: 3,
        handler_idx: 0,
    });
    code.catch_handlers.push(CatchHandler {
        typed_catches: vec![],
        catch_all_addr: Some(6),
    });
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "unusedResult",
        "()V",
        (2, 0),
        vec![
            Const16 { dest: 1, value: 4 },
            computing(),
            computing(),
            MoveResult { dest: 0 },
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "usedResult",
        "()V",
        (2, 0),
        vec![
            Const16 { dest: 1, value: 4 },
            computing(),
            MoveResult { dest: 0 },
            ReturnVoid,
        ],
    );
    add_static_method(
        &mut dex,
        class,
        OWNER,
        "superCall",
        "(Ljava/lang/Object;JI)V",
        (4, 4),
        vec![
            InvokeSuper {
                method: take_wide,
                args: [0, 1, 2, 3].into_iter().collect(),
            },
            ReturnVoid,
        ],
    );
    for (name, proto, words) in [
        ("take", "(I)V", 1),
        ("replacement", "(I)V", 1),
        ("wrong", "(Ljava/lang/String;)V", 1),
        ("instance", "(I)V", 1),
        ("concealed", "(I)V", 1),
        ("consume", "(Ljava/lang/Object;JI)V", 4),
    ] {
        add_static_method(
            &mut dex,
            observer,
            OBSERVER,
            name,
            proto,
            (words, words),
            vec![ReturnVoid],
        );
    }
    let private_id = dex.intern_method(OBSERVER, "concealed", "(I)V").unwrap();
    dex.class_mut(observer)
        .unwrap()
        .class_data
        .as_mut()
        .unwrap()
        .direct_methods
        .iter_mut()
        .find(|method| method.method == private_id)
        .unwrap()
        .access_flags = AccessFlags::PRIVATE | AccessFlags::STATIC;

    let (_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let mut extension = DexFile::new(empty_dex_header(DexVersion::V035));
    let ext_class = extension
        .create_class(
            "Lcom/example/ExtensionCaller;",
            AccessFlags::PUBLIC,
            Some("Ljava/lang/Object;"),
        )
        .unwrap();
    let ext_take = extension.intern_method(OBSERVER, "take", "(I)V").unwrap();
    add_static_method(
        &mut extension,
        ext_class,
        "Lcom/example/ExtensionCaller;",
        "call",
        "(I)V",
        (1, 1),
        vec![
            InvokeStatic {
                method: ext_take,
                args: [0].into_iter().collect(),
            },
            ReturnVoid,
        ],
    );
    apk.add_dex(extension);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "point-queries",
        "pointQueries",
    );
    let report = apk
        .component_mut(0)
        .unwrap()
        .read_entry("assets/point-queries.txt")
        .unwrap()
        .unwrap();
    assert_eq!(
        String::from_utf8(report).unwrap(),
        "copied:1\nsame:1\nconflict:0\npassed:0\nwide:1\nwideCopied:1\nwideBroken:0\nexception:1\nduplicates:2\noverwritten:1\nunknown:0"
    );
    let code = patched_code(&apk, OWNER, "duplicates");
    assert_eq!(
        code.instructions
            .iter()
            .filter(|i| matches!(i, InvokeStatic { .. } | InvokeStaticRange { .. }))
            .count(),
        4
    );
    assert_eq!(static_call_registers(&code), vec![vec![3]; 4]);
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "point-redirects",
        "pointRedirects",
    );
    let wide = patched_code(&apk, OWNER, "wide");
    assert!(matches!(
        wide.instructions[1],
        InvokeStaticRange {
            first_reg: 16,
            count: 4,
            ..
        }
    ));
}

#[test]
fn string_prefix_queries_use_literal_prefixes_and_exclude_extensions() {
    use reseam_apk::reseam_dex::{AccessFlags, Instruction::*};
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let owner = "Lcom/example/Routes;";
    let class = dex
        .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    for (name, text) in [
        ("watch", "reel/watch"),
        ("create", "reel/create"),
        ("embedded", "other/reel/watch"),
        ("other", "browse"),
    ] {
        let string = dex.intern_string(text);
        add_static_method(
            &mut dex,
            class,
            owner,
            name,
            "()V",
            (1, 0),
            vec![
                ConstString { dest: 0, string },
                ConstString { dest: 0, string },
                ReturnVoid,
            ],
        );
    }
    let (_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let mut extension = DexFile::new(empty_dex_header(DexVersion::V035));
    let owner = "Lcom/example/ExtensionRoutes;";
    let class = extension
        .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let string = extension.intern_string("reel/extension");
    add_static_method(
        &mut extension,
        class,
        owner,
        "route",
        "()V",
        (1, 0),
        vec![ConstString { dest: 0, string }, ReturnVoid],
    );
    apk.add_dex(extension);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "string-prefix-queries",
        "stringPrefixQueries",
    );
}

#[test]
fn high16_literals_are_consistent_in_queries_and_mutations() {
    use reseam_apk::reseam_dex::{AccessFlags, Instruction::*};
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    let owner = "Lcom/example/High16;";
    let class = dex
        .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    for (name, instruction) in [
        (
            "positive",
            ConstHigh16 {
                dest: 0,
                value: 1024,
            },
        ),
        ("negative", ConstHigh16 { dest: 0, value: -1 }),
        (
            "wide",
            ConstWideHigh16 {
                dest: 0,
                value: 4096,
            },
        ),
    ] {
        add_static_method(
            &mut dex,
            class,
            owner,
            name,
            "()V",
            (2, 0),
            vec![instruction, ReturnVoid],
        );
    }
    let (_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "high16-literals",
        "high16Literals",
    );
    assert!(matches!(
        patched_code(&apk, owner, "positive").instructions[0],
        ConstHigh16 { value: 2048, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "negative").instructions[0],
        ConstHigh16 { value: -2, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "wide").instructions[0],
        ConstWideHigh16 {
            value: i16::MIN,
            ..
        }
    ));
}

#[test]
fn instance_field_queries_read_lazy_fields_and_observe_edits() {
    use reseam_apk::reseam_dex::{AccessFlags, EncodedField};
    let mut dex = DexFile::new(empty_dex_header(DexVersion::V035));
    for (owner, is_static) in [("LInstance;", false), ("LStatic;", true)] {
        let class = dex
            .create_class(owner, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
            .unwrap();
        let field = dex
            .intern_field(owner, "value", "Ljava/lang/String;")
            .unwrap();
        let field = EncodedField {
            field,
            access_flags: AccessFlags::PUBLIC,
        };
        if is_static {
            dex.class_mut(class).unwrap().add_static_field(field);
        } else {
            dex.class_mut(class).unwrap().add_instance_field(field);
        }
    }
    let (_dir, mut apk) = open_split_test_apk_with(manifest_bytes("1.0-base", None), &[dex]);
    let mut ctx = PatchContext::new(&mut apk);
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    let matches = ctx.find_classes_with_instance_field("Ljava/lang/String;");
    assert_eq!(matches.len(), 1);
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    assert!(ctx.find_classes_with_instance_field("LAbsent;").is_empty());
    let dex = ctx.dex_file_mut(0).unwrap();
    let index = dex.find_class_index("LStatic;").unwrap();
    let field = dex
        .intern_field("LStatic;", "instance", "Ljava/lang/String;")
        .unwrap();
    dex.class_mut(index)
        .unwrap()
        .add_instance_field(EncodedField {
            field,
            access_flags: AccessFlags::PUBLIC,
        });
    assert_eq!(
        ctx.find_classes_with_instance_field("Ljava/lang/String;")
            .len(),
        2
    );
}

#[test]
fn settings_pages_merge_selected_contributions_and_keep_nested_ancestors() {
    let bundle_file = write_bundle_reseam();
    let bundle = BundleArchive::open(&bundle_file.path)
        .unwrap()
        .load()
        .unwrap();
    let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
    // Run twice against the same loaded host: pages from the first selection must not leak.
    for (patch, expected_pages, expected_keys) in [
        (
            "navigationSecondPatch",
            vec!["general", "media", "quality"],
            vec!["quality.first", "quality.second"],
        ),
        (
            "navigationFirstPatch",
            vec!["media", "quality"],
            vec!["quality.first"],
        ),
    ] {
        let (_dir, mut apk) = open_split_test_apk();
        let selection = PatchSelection {
            enable: [format!("runtime-test-bundle/app.reseam.test.{patch}")].into(),
            ..Default::default()
        };
        let results = engine::apply_patches(
            &mut PatchContext::new(&mut apk),
            &patches,
            &selection,
            |_| {},
        )
        .unwrap();
        assert!(
            results
                .iter()
                .all(|r| !matches!(r.status, PatchStatus::Failed { .. }))
        );
        let bytes = apk
            .component_mut(0)
            .unwrap()
            .read_entry("assets/reseam/settings.json")
            .unwrap()
            .unwrap();
        let schema: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let pages = schema["pages"].as_array().unwrap();
        assert_eq!(
            pages
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_pages
        );
        let child = pages.iter().find(|p| p["id"] == "quality").unwrap();
        assert_eq!(child["parent"], "media");
        let sections = schema["sections"].as_array().unwrap();
        assert!(sections[0]["page"].is_null());
        assert_eq!(sections[0]["settings"][0]["key"], "root.enabled");
        let quality: Vec<_> = sections.iter().filter(|s| s["page"] == "quality").collect();
        assert_eq!(quality.len(), 1);
        assert_eq!(
            quality[0]["settings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["key"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_keys
        );
        assert_eq!(quality[0]["settings"][0]["default"], true);
    }
}
