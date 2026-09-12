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

    assert_eq!(results.len(), 13);
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
                    return (calls, Some(registers[*src as usize]))
                }
                ReturnWide { src } => {
                    return (
                        calls,
                        Some(registers[*src as usize] | (registers[*src as usize + 1] << 32)),
                    )
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
        self as dex, AccessFlags, CodeItem, DexFile, DexHeader, DexVersion, EncodedMethod,
        Instruction::*,
    };

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
    let (_apk_dir, mut apk) = open_split_test_apk();
    // Round trip before and after patching to exercise lazy decoding and writing.
    apk.add_dex(
        dex::parse(
            &dex::write(&dex).unwrap(),
            ParseOptions {
                lazy: true,
                ..Default::default()
            },
        )
        .unwrap(),
    );
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
fn same_named_patches_keep_independent_identity_options_dependencies_and_settings() {
    use reseam_patcher::engine::{validate_patches, PatchIndex};
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
    assert!(patches[index.resolve(second, None).unwrap()]
        .spec()
        .dependencies
        .iter()
        .any(|id| id == first));
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
    let mut first_options = PatchOptions::default();
    first_options.set("marker", OptionValue::String("one".into()));
    let mut second_options = PatchOptions::default();
    second_options.set("marker", OptionValue::String("two".into()));
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
    assert!(results
        .iter()
        .filter(|r| r.patch == first || r.patch == second)
        .all(|r| matches!(r.status, PatchStatus::Skipped { .. })));

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
