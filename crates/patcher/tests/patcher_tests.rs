// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#![expect(clippy::unwrap_used, reason = "fixture operations must succeed")]

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use reseam_apk::axml::AxmlAttribute;
use reseam_apk::{ApkFile, ResValue, ResourceTable};
use reseam_dex::{DexFile, DexHeader, DexVersion, ParseOptions};
use reseam_patcher::bundle::{BUNDLE_FORMAT_VERSION, BundleArchive, BundleKey, pack};
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, PatchSelection, PatchStatus};
use reseam_patcher::options::OptionValue;
use reseam_patcher::{ApkVersion, Patch, PatchPreset};

macro_rules! dex {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/", $name, ".dex")).as_slice()
    };
}

fn entry(apk: &mut ApkFile, component: usize, name: &str) -> Option<Vec<u8>> {
    apk.read_component_entry(component, name).unwrap()
}

static FIXTURE_JAR: OnceLock<PathBuf> = OnceLock::new();

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_checked(cmd: &mut Command, context: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("{context}: failed to spawn: {e}"));
    assert!(
        output.status.success(),
        "{context} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
                    .arg(&fixture_dir)
                    .arg("--no-daemon")
                    .arg("-Dorg.gradle.jvmargs=-Xmx1536m")
                    .arg("patchJar"),
                "build kotlin runtime test bundle",
            );

            fixture_dir.join("build/reseam/reseam-test-patches.jar")
        })
        .clone()
}

const TEST_SIGNING_SEED: [u8; 32] = [0x42; 32];

struct TestBundle {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn write_bundle_reseam() -> TestBundle {
    let tmp = tempfile::tempdir().unwrap();
    let out_path = tmp.path().join("runtime-test-bundle.reseam");
    fs::copy(
        build_fixture_jar(),
        tmp.path().join("reseam-test-patches.jar"),
    )
    .unwrap();
    fs::write(
        tmp.path().join("manifest.toml"),
        format!(
            "[bundle]\nname = 'runtime-test-bundle'\nlicense = 'GPL-3.0'\nformat_version = {BUNDLE_FORMAT_VERSION}\n"
        ),
    )
    .unwrap();
    let key = tmp.path().join("bundle-signing.key");
    fs::write(&key, TEST_SIGNING_SEED).unwrap();
    pack(tmp.path(), None, &BundleKey::read(&key).unwrap(), &out_path).unwrap();

    TestBundle {
        _dir: tmp,
        path: out_path,
    }
}

fn manifest_with_application(class: &str) -> Vec<u8> {
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="1.0-base"><application android:name="{class}" /></manifest>"#
    ), None).unwrap()
}

fn manifest_bytes(version_name: &str, split_name: Option<&str>) -> Vec<u8> {
    let split_attr = split_name
        .map(|name| format!(r#" split="{name}""#))
        .unwrap_or_default();
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
    ), None).unwrap()
}

const LAYOUT_PATH: &str = "res/Qy.xml";

fn write_apk(path: &Path, manifest: &[u8], extra_entries: &[(&str, &[u8])]) {
    let file = File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    writer.start_file("AndroidManifest.xml", options).unwrap();
    writer.write_all(manifest).unwrap();

    for (name, data) in extra_entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }

    writer.finish().unwrap();
}

fn open_dex_apk(dex_files: &[&[u8]]) -> (tempfile::TempDir, ApkFile) {
    open_split_test_apk_with(&manifest_bytes("1.0-base", None), dex_files)
}

fn open_split_test_apk() -> (tempfile::TempDir, ApkFile) {
    open_dex_apk(&[])
}

/** An app with an `Application`, which a settings host needs to start the app runtime. */
fn open_settings_test_apk() -> (tempfile::TempDir, ApkFile) {
    open_split_test_apk_with(
        &manifest_with_application("com.example.App"),
        &[dex!("entry_app"), dex!("entry_base")],
    )
}

const SETTINGS_SCHEMA: &str = "assets/reseam/runtime-test-bundle/settings.json";
const SETTINGS_STRINGS: &str = "assets/reseam/runtime-test-bundle/strings/TestStrings/default.json";
const RUNTIME_STRINGS: &str = "assets/reseam/runtime/strings/default.json";

fn open_split_test_apk_with(
    base_manifest: &[u8],
    dex_files: &[&[u8]],
) -> (tempfile::TempDir, ApkFile) {
    let tmp = tempfile::tempdir().unwrap();
    let base_path = tmp.path().join("base.apk");
    let split_path = tmp.path().join("config.apk");
    let layout = include_bytes!("fixtures/layout.xml").as_slice();
    let base_resources = include_bytes!("fixtures/base.arsc").as_slice();
    let split_resources = include_bytes!("fixtures/split.arsc").as_slice();

    let dex_bytes: Vec<(String, &[u8])> = dex_files
        .iter()
        .enumerate()
        .map(|(index, dex)| {
            let name = match index {
                0 => "classes.dex".to_string(),
                n => format!("classes{}.dex", n + 1),
            };
            (name, *dex)
        })
        .collect();
    let mut base_entries: Vec<(&str, &[u8])> =
        vec![("resources.arsc", base_resources), (LAYOUT_PATH, layout)];
    base_entries.extend(
        dex_bytes
            .iter()
            .map(|(name, bytes)| (name.as_str(), *bytes)),
    );

    write_apk(&base_path, base_manifest, &base_entries);
    write_apk(
        &split_path,
        &manifest_bytes("1.0-split", Some("config.test")),
        &[("resources.arsc", split_resources)],
    );

    let apk = ApkFile::open_split(
        &base_path,
        &[split_path.as_path()],
        ParseOptions {
            classes: reseam_dex::Loading::Deferred,
            ..ParseOptions::default()
        },
    )
    .unwrap();

    (tmp, apk)
}

fn manifest_contains_permission(apk: &ApkFile, permission: &str) -> bool {
    let manifest = apk.base().manifest();
    (0..manifest.events().len()).any(|i| {
        manifest.element_name(i).as_deref() == Some("uses-permission")
            && manifest
                .attribute_named(i, "name")
                .is_some_and(|attr| manifest.attribute_string(attr).as_deref() == Some(permission))
    })
}

#[test]
fn kotlin_bundle_executes_against_runtime_api() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
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
        preset: PatchPreset::None,
        enable: ["runtime-api", "dependent-runtime", "owned-callbacks"]
            .map(String::from)
            .into(),
        options: [("runtime-api".to_string(), options)].into(),
        ..Default::default()
    };

    let results = apply(&mut ctx, &patches, &selection).unwrap();

    let statuses = results
        .iter()
        .map(|result| (result.patch.as_str(), &result.status))
        .collect::<std::collections::HashMap<_, _>>();
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
    assert!(!statuses.contains_key("runtime-test-bundle/app.reseam.test.requiredOption"));

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
                    .unwrap()
                    .map(std::borrow::Cow::into_owned)
            })
    };
    assert_eq!(
        split_label(&mut apk, 1).as_deref(),
        Some("Split patched by runtime")
    );
    assert_eq!(split_label(&mut apk, 0), None);

    let mut entry = |index: usize, name: &str| entry(&mut apk, index, name);
    assert_eq!(entry(0, "assets/base-marker.txt"), Some(b"base".to_vec()));
    assert_eq!(entry(1, "assets/split-marker.txt"), Some(b"split".to_vec()));
    assert_eq!(
        entry(1, "assets/dependent-marker.txt"),
        Some(b"dependent".to_vec())
    );
    assert_eq!(entry(0, "assets/split-marker.txt"), None);
    for name in ["assets/owned-execute.txt", "assets/owned-finalize.txt"] {
        assert_eq!(entry(0, name), Some(b"original".to_vec()));
    }
}

enum SelectionOutcome {
    Applied(&'static [&'static str]),
    NotRun(&'static str),
    MissingBundle,
    MissingOption,
}

struct SelectionCase {
    selected: Option<&'static str>,
    token: Option<&'static str>,
    expected: SelectionOutcome,
}

impl SelectionCase {
    const fn new(selected: Option<&'static str>, expected: SelectionOutcome) -> Self {
        Self {
            selected,
            token: None,
            expected,
        }
    }
}

const SELECTION_CASES: [SelectionCase; 9] = [
    SelectionCase::new(None, SelectionOutcome::NotRun("universalMarker")),
    SelectionCase::new(
        Some("universal-marker"),
        SelectionOutcome::Applied(&["universalMarker"]),
    ),
    SelectionCase::new(
        Some("uses-internal"),
        SelectionOutcome::Applied(&["internalHelper", "usesInternal"]),
    ),
    SelectionCase::new(
        Some("installed-version-code"),
        SelectionOutcome::Applied(&["installedVersionCode"]),
    ),
    SelectionCase::new(Some("other-version-code"), SelectionOutcome::Applied(&[])),
    SelectionCase::new(None, SelectionOutcome::NotRun("needsOtherBundle")),
    SelectionCase::new(Some("needs-other-bundle"), SelectionOutcome::MissingBundle),
    SelectionCase::new(Some("required-option"), SelectionOutcome::MissingOption),
    SelectionCase {
        selected: Some("required-option"),
        token: Some("provided"),
        expected: SelectionOutcome::Applied(&["requiredOption"]),
    },
];

#[test]
fn kotlin_patch_selection_respects_dependencies_and_required_options() {
    use reseam_patcher::error::PatcherError;

    let bundle = loaded_test_bundle();
    let patches: Vec<_> = bundle.patches().iter().collect();
    for SelectionCase {
        selected,
        token,
        expected,
    } in SELECTION_CASES
    {
        let (_directory, mut apk) = open_split_test_apk();
        let selection = PatchSelection {
            preset: PatchPreset::None,
            enable: selected.map(String::from).into_iter().collect(),
            options: token
                .map(|value| {
                    (
                        "required-option".to_owned(),
                        [("token".to_owned(), OptionValue::Text(value.to_owned()))].into(),
                    )
                })
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let result = apply(&mut PatchContext::new(&mut apk), &patches, &selection);
        match expected {
            SelectionOutcome::MissingBundle => assert!(matches!(result,
                Err(PatcherError::MissingBundle { patch, dependency, bundle })
                    if patch == "runtime-test-bundle/app.reseam.test.needsOtherBundle"
                        && dependency == "other-bundle/app.reseam.other.helper"
                        && bundle == "other-bundle")),
            SelectionOutcome::MissingOption => assert!(matches!(result,
                Err(PatcherError::MissingRequiredOption { key, .. }) if key == "token")),
            SelectionOutcome::NotRun(id) => {
                assert!(!result.unwrap().iter().any(|item| item.patch.ends_with(id)));
            }
            SelectionOutcome::Applied(ids) => {
                let results = result.unwrap();
                let applied: Vec<_> = results
                    .iter()
                    .filter(|item| item.status == PatchStatus::Applied)
                    .map(|item| item.patch.rsplit('.').next().unwrap())
                    .collect();
                assert_eq!(applied, ids);
                if let Some(helper) = results.iter().find(|item| {
                    item.patch.ends_with("internalHelper") && item.status == PatchStatus::Applied
                }) {
                    assert!(helper.hidden);
                    assert_eq!(
                        helper.required_by,
                        ["runtime-test-bundle/app.reseam.test.usesInternal"]
                    );
                    assert_eq!(
                        apk.read_component_entry(0, "assets/internal-marker.txt")
                            .unwrap(),
                        Some(b"internal".to_vec())
                    );
                }
                if selected == Some("universal-marker") {
                    assert_eq!(
                        apk.read_component_entry(0, "assets/universal-marker.txt")
                            .unwrap(),
                        Some(b"universal".to_vec())
                    );
                }
            }
        }
    }
}

#[test]
fn when_instance_of_branches_on_the_runtime_type() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("instance_branch")]);
    apply_fixture(&mut apk, "when-instance-of", "whenInstanceOfBranch");

    let code = patched_code(&apk, OWNER, "typed");
    assert_eq!(hook_calls(&code, &[1]), (vec![vec![1]], None));
    assert_eq!(hook_calls(&code, &[0]), (vec![vec![2]], None));
}

#[test]
fn point_before_runs_on_branches_into_the_instruction() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("branch_entry")]);
    apply_fixture(&mut apk, "before-join", "beforeJoin");

    let code = patched_code(&apk, OWNER, "joinedBefore");
    assert_eq!(
        hook_calls(&code, &[0]),
        (vec![vec![1], vec![9], vec![2], vec![3]], None)
    );
    assert_eq!(
        hook_calls(&code, &[1]),
        (vec![vec![1], vec![4], vec![9], vec![4], vec![3]], None)
    );
}

#[test]
fn skip_when_guards_a_call_on_every_path_into_it() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("guarded_calls")]);
    apply_fixture(&mut apk, "skip-when", "skipWhenCall");

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

#[test]
fn a_replaced_body_returns_from_its_conditional() {
    const OWNER: &str = "Lcom/example/AnchorHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("replacement_branch")]);
    apply_fixture(&mut apk, "replace-both-return", "replaceBothReturn");
    for (condition, expected) in [(0, 20), (1, 10)] {
        assert_eq!(
            hook_calls(&patched_code(&apk, OWNER, "replaced"), &[condition]),
            (vec![], Some(expected))
        );
    }
}

fn anchor_host_apk() -> (tempfile::TempDir, ApkFile) {
    open_dex_apk(&[dex!("anchor_host")])
}

fn kotlin_patch_failure(patch: &str, id: &str) -> PatchStatus {
    let (_apk_dir, mut apk) = anchor_host_apk();
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let selection = PatchSelection {
        enable: [patch.to_string()].into(),
        ..Default::default()
    };
    let results = apply(&mut PatchContext::new(&mut apk), &patches, &selection).unwrap();
    let result = results
        .iter()
        .find(|r| r.patch.ends_with(id))
        .unwrap_or_else(|| panic!("{id} not in results"));
    result.status.clone()
}

#[test]
fn invalid_emitted_operations_fail_the_patch() {
    for (patch, id) in [
        ("assign-field-read", "assignFieldRead"),
        ("after-return", "afterReturn"),
        ("private-field-access", "privateFieldAccess"),
        ("ext-method-missing", "extMethodMissing"),
        ("call-arity", "callArity"),
        ("replace-falls-through", "replaceFallsThrough"),
        ("wide-condition", "wideCondition"),
        ("primitive-null", "primitiveNull"),
        ("mixed-equality", "mixedEquality"),
        ("primitive-instance", "primitiveInstance"),
        ("primitive-instance-type", "primitiveInstanceType"),
        ("primitive-construction", "primitiveConstruction"),
        ("invalid-constructor-return", "invalidConstructorReturn"),
    ] {
        assert!(
            matches!(kotlin_patch_failure(patch, id), PatchStatus::Failed { .. }),
            "{patch}"
        );
    }
}

// Distinct incoming words expose lost arguments and overlapping wide moves.
fn hook_calls(code: &reseam_dex::CodeItem, incoming: &[i64]) -> (Vec<Vec<i64>>, Option<i64>) {
    use reseam_dex::Instruction::*;
    let mut registers = vec![-1; usize::from(code.registers_size())];
    registers[usize::from(code.registers_size() - code.ins_size())..].copy_from_slice(incoming);
    let mut calls = Vec::new();
    let offsets: Vec<u32> = code
        .instructions()
        .iter()
        .scan(0, |addr, insn| {
            let current = *addr;
            *addr += insn.code_units();
            Some(current)
        })
        .collect();
    let target = |pc: usize, displacement: i32| {
        let address = offsets[pc].checked_add_signed(displacement).unwrap();
        offsets
            .iter()
            .position(|offset| *offset == address)
            .unwrap()
    };
    let mut pc = 0;
    for _ in 0..code.instructions().len() * 2 {
        let insn = &code.instructions()[pc];
        let movement = fixture_movement(insn);
        let wide_constant = match insn {
            ConstWide16 { dest, .. }
            | ConstWide32 { dest, .. }
            | ConstWide { dest, .. }
            | ConstWideHigh16 { dest, .. } => Some((*dest, insn.literal().unwrap())),
            _ => None,
        };
        if let Some((dest, value)) = wide_constant {
            registers[usize::from(dest)] = i64::from(value as u32);
            registers[usize::from(dest) + 1] = i64::from((value >> 32) as u32);
        } else if let Some(movement) = movement {
            registers.copy_within(
                movement.source..movement.source + movement.words,
                movement.destination,
            );
        } else if let Some(arguments) = fixture_invoke_arguments(insn) {
            calls.push(
                arguments
                    .into_iter()
                    .map(|register| registers[register])
                    .collect(),
            );
        } else if let Some((destination, value)) = fixture_scalar_write(insn, &registers) {
            registers[destination] = value;
        } else {
            match insn {
                IfEq { a, b, offset } | IfNe { a, b, offset }
                    if (registers[usize::from(*a)] == registers[usize::from(*b)])
                        == matches!(insn, IfEq { .. }) =>
                {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                IfEqz { a, offset } | IfNez { a, offset }
                    if (registers[usize::from(*a)] == 0) == matches!(insn, IfEqz { .. }) =>
                {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto { offset } => {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto16 { offset } => {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto32 { offset } => {
                    pc = target(pc, *offset);
                    continue;
                }
                Nop | IfEqz { .. } | IfNez { .. } | IfEq { .. } | IfNe { .. } => {}
                Return { src } | ReturnObject { src } => {
                    return (calls, Some(registers[usize::from(*src)]));
                }
                ReturnWide { src } => {
                    return (
                        calls,
                        Some(
                            registers[usize::from(*src)] | (registers[usize::from(*src) + 1] << 32),
                        ),
                    );
                }
                ReturnVoid => return (calls, None),
                _ => panic!("unexpected fixture instruction: {insn:?}"),
            }
        }
        pc += 1;
    }
    panic!("fixture did not return");
}

fn fixture_scalar_write(
    instruction: &reseam_dex::Instruction,
    registers: &[i64],
) -> Option<(usize, i64)> {
    use reseam_dex::Instruction::*;
    use std::cmp::Ordering;
    let wide = |register: u8| {
        registers[usize::from(register)] as u64
            | ((registers[usize::from(register) + 1] as u64) << 32)
    };
    let compare = |order: Option<Ordering>| match order {
        Some(Ordering::Equal) => 0,
        Some(Ordering::Greater) => 1,
        None if matches!(instruction, CmpGFloat { .. } | CmpGDouble { .. }) => 1,
        Some(Ordering::Less) | None => -1,
    };
    let (dest, value) = match *instruction {
        Const4 { dest, value } => (dest, i64::from(value)),
        Const16 { dest, value } => (dest, i64::from(value)),
        Const { dest, value } => (dest, i64::from(value)),
        ConstHigh16 { dest, value } => (dest, i64::from(value) << 16),
        // Fixture references are either null or instances of the queried type.
        InstanceOf { dest, ref_, .. } => (dest, i64::from(registers[usize::from(ref_)] != 0)),
        CmpLong { dest, a, b } => (dest, compare(Some((wide(a) as i64).cmp(&(wide(b) as i64))))),
        CmpLFloat { dest, a, b } | CmpGFloat { dest, a, b } => (
            dest,
            compare(
                f32::from_bits(registers[usize::from(a)] as u32)
                    .partial_cmp(&f32::from_bits(registers[usize::from(b)] as u32)),
            ),
        ),
        CmpLDouble { dest, a, b } | CmpGDouble { dest, a, b } => (
            dest,
            compare(f64::from_bits(wide(a)).partial_cmp(&f64::from_bits(wide(b)))),
        ),
        _ => return None,
    };
    Some((usize::from(dest), value))
}

fn fixture_invoke_arguments(instruction: &reseam_dex::Instruction) -> Option<Vec<usize>> {
    use reseam_dex::Instruction::*;
    match instruction {
        InvokeStatic { args, .. }
        | InvokeVirtual { args, .. }
        | InvokeDirect { args, .. }
        | InvokeSuper { args, .. }
        | InvokeInterface { args, .. } => Some(args.iter().map(|r| usize::from(*r)).collect()),
        InvokeStaticRange {
            first_reg, count, ..
        }
        | InvokeVirtualRange {
            first_reg, count, ..
        }
        | InvokeDirectRange {
            first_reg, count, ..
        }
        | InvokeSuperRange {
            first_reg, count, ..
        }
        | InvokeInterfaceRange {
            first_reg, count, ..
        } => {
            Some((usize::from(*first_reg)..usize::from(*first_reg) + usize::from(*count)).collect())
        }
        _ => None,
    }
}

struct FixtureMovement {
    destination: usize,
    source: usize,
    words: usize,
}

fn fixture_movement(instruction: &reseam_dex::Instruction) -> Option<FixtureMovement> {
    use reseam_dex::Instruction::*;
    let (destination, source) = match *instruction {
        Move { dest, src } | MoveObject { dest, src } | MoveWide { dest, src } => {
            (usize::from(dest), usize::from(src))
        }
        MoveFrom16 { dest, src }
        | MoveObjectFrom16 { dest, src }
        | MoveWideFrom16 { dest, src } => (usize::from(dest), usize::from(src)),
        Move16 { dest, src } | MoveObject16 { dest, src } | MoveWide16 { dest, src } => {
            (usize::from(dest), usize::from(src))
        }
        _ => return None,
    };
    let words = if matches!(
        instruction,
        MoveWide { .. } | MoveWideFrom16 { .. } | MoveWide16 { .. }
    ) {
        2
    } else {
        1
    };
    Some(FixtureMovement {
        destination,
        source,
        words,
    })
}

#[test]
fn after_hooks_preserve_entry_arguments_when_parameter_registers_are_reused() {
    let owner = "Lcom/example/HookTarget;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("entry_values")]);
    apply_fixture(&mut apk, "after-entry-values", "afterEntryValues");
    for name in [
        "invokeGrowth",
        "getFeatureSwitchValue",
        "receiver",
        "temporaryReuse",
        "resultOnly",
    ] {
        let code = patched_code(&apk, owner, name);
        match name {
            "invokeGrowth" => {
                assert_eq!(
                    hook_calls(&code, &[101, 102, 103, 104, 105]),
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
                        hook_calls(&code, &[101, 102, 103, 104, 105, last]),
                        (
                            vec![vec![42, 101, 102, 103, 104, 105, last, 101, result]],
                            Some(result)
                        )
                    );
                }
            }
            "receiver" => assert_eq!(hook_calls(&code, &[201]), (vec![vec![201]], None)),
            "temporaryReuse" => {
                let mut expected = Vec::new();
                for value in 0..32 {
                    expected.push(vec![value, 0]);
                    expected.push(vec![value]);
                }
                expected.push(vec![0x9abc_def0, 0x1234_5678]);
                for condition in [0, 1] {
                    assert_eq!(
                        hook_calls(&code, &[condition]),
                        (expected.clone(), Some(0x1234_5678_9abc_def0))
                    );
                }
            }
            "resultOnly" => {
                assert_eq!(hook_calls(&code, &[301]), (vec![], Some(42)));
            }
            other => panic!("unexpected method {other}"),
        }
    }
}

#[test]
fn replace_strings_containing_or_starting_with_rewrites_only_what_transform_returns() {
    use reseam_dex::Instruction::ConstString;

    const REWRITTEN: &str = "content://app.reseam.gsf.gservices/prefix";
    const ACTION: &str = "com.google.android.gsf.intent.SYNC";

    let owner = "Lcom/example/StringHolder;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("embedded_strings")]);
    apply_fixture(&mut apk, "embedded-strings", "embeddedStrings");
    assert_eq!(
        apk.read_component_entry(0, "assets/embedded-strings.txt")
            .unwrap(),
        Some(b"2|1".to_vec()),
        "two URIs contain the value; only the action starts with it",
    );

    let dex = written_dex(&apk, 0);
    for (name, expected) in [("uris", vec![REWRITTEN, ACTION]), ("more", vec![REWRITTEN])] {
        let code = read_method(&dex, owner, name).unwrap().code.unwrap();
        let constants: Vec<_> = code
            .instructions()
            .iter()
            .filter_map(|instruction| match instruction {
                ConstString { string, .. } => Some(dex.strings().get(*string).into_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(constants, expected);
    }
}

#[test]
fn same_named_patches_keep_independent_identity_options_dependencies_and_settings() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let first = "runtime-test-bundle/app.reseam.test.firstAds";
    let second = "runtime-test-bundle/app.reseam.test.secondAds";
    let (_apk_dir, mut apk) = open_settings_test_apk();
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
    let results = apply(&mut PatchContext::new(&mut apk), &patches, &selection).unwrap();
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
            apk.read_component_entry(0, path).unwrap().unwrap(),
            expected
        );
    }
    let schema = entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap();
    let schema: serde_json::Value = serde_json::from_slice(&schema).unwrap();
    let sections = schema["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["title"]["name"], "section_first");
    assert_eq!(sections[1]["title"]["name"], "section_second");
    assert!(
        schema["about"].is_null(),
        "a host without about leaves the card bare"
    );
}

fn apply_fixture(apk: &mut ApkFile, name: &str, id: &str) {
    let bundle = loaded_test_bundle();
    run_one_patch(
        &mut PatchContext::new(apk),
        &bundle.patches().iter().collect::<Vec<_>>(),
        name,
        id,
    );
}

fn run_one_patch(ctx: &mut PatchContext<'_>, patches: &[&Patch], name: &str, id: &str) {
    let results = apply(
        ctx,
        patches,
        &PatchSelection {
            enable: [name.to_string()].into(),
            ..Default::default()
        },
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
    load_all(&write_bundle_reseam().path)
}

/// Every patch of the bundle at `path`, so one load serves selections a test picks later.
fn load_all(path: &Path) -> reseam_patcher::bundle::PatchBundle {
    let archive = BundleArchive::open(path).unwrap();
    let ids = archive
        .patches()
        .iter()
        .map(|spec| spec.id.clone())
        .collect();
    archive.load(&ids).unwrap()
}

/// Plans `selection` for the APK in `ctx` and applies it, as a host run does.
fn apply(
    ctx: &mut PatchContext<'_>,
    patches: &[&Patch],
    selection: &PatchSelection,
) -> reseam_patcher::error::Result<Vec<engine::PatchResult>> {
    let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
    let plan = engine::ResolvedPlan::resolve(
        &specs,
        selection,
        engine::Delivery::Install,
        ctx.apk().package_name().as_deref(),
        ApkVersion {
            name: ctx.apk().version_name().as_deref(),
            code: ctx.apk().version_code(),
        },
    )?;
    engine::apply_patches(ctx, &specs, plan, patches, |_| {}).map(engine::AppliedRun::into_results)
}

#[test]
fn a_patch_taken_back_takes_back_what_it_registered_for_a_finalizer() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let selection = PatchSelection {
        preset: PatchPreset::None,
        enable: ["registers-and-fails", "registers-and-applies"]
            .map(String::from)
            .into(),
        ..Default::default()
    };
    for _ in 0..2 {
        let (_apk_dir, mut apk) = open_split_test_apk();
        let results = apply(&mut PatchContext::new(&mut apk), &patches, &selection).unwrap();
        let status = |id: &str| {
            results
                .iter()
                .find(|result| result.patch == format!("runtime-test-bundle/app.reseam.test.{id}"))
                .map(|result| std::mem::discriminant(&result.status))
        };
        assert_eq!(
            status("registersAndFails"),
            Some(std::mem::discriminant(&PatchStatus::Failed {
                reason: String::new()
            }))
        );
        assert_eq!(
            status("registersAndApplies"),
            Some(std::mem::discriminant(&PatchStatus::Applied))
        );
        assert_eq!(
            apk.read_component_entry(0, "assets/registrations.txt")
                .unwrap(),
            Some(b"applied".to_vec())
        );
    }
}

fn invoked_by(dex: &DexFile, owner: &str, name: &str) -> Vec<String> {
    let method = read_method(dex, owner, name).unwrap();
    method
        .code
        .as_ref()
        .unwrap()
        .instructions()
        .iter()
        .filter_map(reseam_dex::Instruction::method_ref)
        .map(|idx| {
            let id = dex.method_id(idx);
            format!(
                "{}->{}",
                dex.type_descriptor(id.class),
                dex.strings().get(id.name)
            )
        })
        .collect()
}

fn dex_bytes(dex: &DexFile) -> Vec<u8> {
    let (spooled, _) = reseam_dex::write_spooled(dex, None).unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut spooled.reader(), &mut bytes).unwrap();
    bytes
}

fn written_dex(apk: &ApkFile, index: usize) -> DexFile {
    reseam_dex::parse(
        &dex_bytes(apk.dex().get(index).unwrap()),
        ParseOptions::default(),
    )
    .unwrap()
}

const APP_ENTRY_HOOK: &str = "Lapp/reseam/AppEntry;->onCreate";

#[test]
fn app_entry_unseals_the_final_on_create_it_overrides() {
    use reseam_dex::AccessFlags;

    const BASE: &str = "Lcom/example/AppBase;";
    const APP: &str = "Lcom/example/App;";

    let (_apk_dir, mut apk) = open_split_test_apk_with(
        &manifest_with_application("com.example.App"),
        &[dex!("entry_app"), dex!("entry_base")],
    );
    apply_fixture(&mut apk, "app-entry-hook", "appEntryHook");

    assert_eq!(
        invoked_by(&written_dex(&apk, 1), APP, "onCreate"),
        [APP_ENTRY_HOOK, "Lcom/example/AppBase;->onCreate"],
        "the added onCreate must call the hook, then the inherited onCreate"
    );
    assert_eq!(
        invoked_by(&written_dex(&apk, 0), "Lapp/reseam/AppEntry;", "onCreate"),
        ["Lcom/example/Observer;->started"]
    );
    assert!(
        !method_flags(&written_dex(&apk, 0), BASE, "onCreate")
            .unwrap()
            .contains(AccessFlags::FINAL),
        "the inherited onCreate is still final, so the override is a LinkageError"
    );
}

#[test]
fn app_entry_follows_an_application_swapped_after_the_hook_was_added() {
    assert_app_entry_follows_swapped_application(&manifest_with_application(
        "com.example.WrapperApp",
    ));
}

/// Protected manifests scramble attribute name strings; the platform reads `android:` attributes by
/// resource ID, so the SDK must find, read and replace them the same way.
#[test]
fn app_entry_follows_an_application_swapped_in_a_manifest_with_scrambled_attribute_names() {
    let manifest = manifest_with_application("com.example.WrapperApp");
    let at = manifest
        .windows(4)
        .position(|window| window == b"name")
        .unwrap();
    let scrambled = [&manifest[..at], b"zzzz", &manifest[at + 4..]].concat();
    assert_app_entry_follows_swapped_application(&scrambled);
}

fn assert_app_entry_follows_swapped_application(manifest: &[u8]) {
    const REAL: &str = "Lcom/example/RealApp;";
    const WRAPPER: &str = "Lcom/example/WrapperApp;";

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest, &[dex!("wrapped_application")]);
    apply_fixture(&mut apk, "unwrap-application", "unwrapApplication");
    let written =
        reseam_apk::AxmlDocument::parse(&entry(&mut apk, 0, "AndroidManifest.xml").unwrap())
            .unwrap();
    let application = written.find_element("application").unwrap();
    assert_eq!(
        written.attributes(application).len(),
        1,
        "the swap must replace android:name, not add a second one"
    );
    assert_eq!(
        written.application_name().as_deref(),
        Some("com.example.RealApp")
    );

    let written = written_dex(&apk, 0);
    assert_eq!(
        invoked_by(&written, REAL, "onCreate"),
        [APP_ENTRY_HOOK, "Landroid/app/Application;->onCreate"],
        "the hook must run from the Application the final manifest names"
    );
    assert!(
        method_flags(&written, WRAPPER, "onCreate").is_none(),
        "the wrapper the app no longer starts must not receive the hook"
    );
}

#[test]
fn app_entry_fails_the_patch_when_the_manifest_names_no_application() {
    let (_apk_dir, mut apk) =
        open_dex_apk(&[&dex_bytes(&DexFile::new(DexHeader::new(DexVersion::V035)))]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let results = apply(
        &mut PatchContext::new(&mut apk),
        &patches,
        &PatchSelection {
            enable: ["app-entry-hook".to_string()].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let status = &results
        .iter()
        .find(|result| result.patch.ends_with(".appEntryHook"))
        .unwrap()
        .status;
    assert!(matches!(status, PatchStatus::Failed { .. }), "{status:?}");
}

fn method_flags(dex: &DexFile, owner: &str, name: &str) -> Option<reseam_dex::AccessFlags> {
    read_method(dex, owner, name).map(|method| method.access_flags)
}

#[test]
fn enum_constants_resolve_by_name_through_obfuscated_fields() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("obfuscated_enum")]);
    apply_fixture(&mut apk, "enum-constants", "enumConstants");

    assert_eq!(
        apk.read_component_entry(0, "assets/enum-constants.txt")
            .unwrap(),
        Some(b"Lcom/example/Plan;|c|d".to_vec()),
        "R8 renamed PREMIUM and FAMILY to c and d; the class query finds the enum by its names"
    );
    let written = written_dex(&apk, 0);
    let code = read_method(&written, "Lcom/example/Account;", "plan")
        .unwrap()
        .code
        .unwrap();
    let read = code
        .instructions()
        .iter()
        .find_map(reseam_dex::Instruction::field_ref)
        .map(|idx| {
            written
                .strings()
                .get(written.field_id(idx).name)
                .into_owned()
        });
    assert_eq!(read.as_deref(), Some("c"));
}

#[test]
fn field_access_queries_find_readers_writers_and_the_one_field_a_method_reads() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("obfuscated_enum")]);
    apply_fixture(&mut apk, "field-accesses", "fieldAccesses");

    assert_eq!(
        apk.read_component_entry(0, "assets/field-accesses.txt")
            .unwrap(),
        Some(b"Lcom/example/Plan;.<clinit>|Lcom/example/Plan;.a|Lcom/example/Plan;.b".to_vec()),
    );
}

#[test]
fn xml_files_are_found_by_the_attributes_their_elements_carry() {
    let (_apk_dir, mut apk) = open_dex_apk(&[]);
    apply_fixture(&mut apk, "xml-files-with", "xmlFilesWith");

    assert_eq!(
        apk.read_component_entry(0, "assets/xml-files-with.txt")
            .unwrap(),
        Some(format!("{LAYOUT_PATH}|").into_bytes()),
    );
}

#[test]
fn enum_constants_resolve_when_optimizers_bypass_the_enum_constructor() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("optimized_enum")]);
    apply_fixture(
        &mut apk,
        "optimized-enum-constants",
        "optimizedEnumConstants",
    );

    assert_eq!(
        apk.read_component_entry(0, "assets/optimized-enum-constants.txt")
            .unwrap(),
        Some(b"Lcom/example/License;|c|Lcom/example/Tier;|c".to_vec()),
        "License constants call Enum.<init> directly; Tier constants are built by their bodies' subclasses"
    );
    let written = written_dex(&apk, 0);
    for (method, owner) in [
        ("license", "Lcom/example/License;"),
        ("tier", "Lcom/example/Tier;"),
    ] {
        let code = read_method(&written, "Lcom/example/Account;", method)
            .unwrap()
            .code
            .unwrap();
        let read = code
            .instructions()
            .iter()
            .find_map(reseam_dex::Instruction::field_ref)
            .map(|idx| {
                let field = written.field_id(idx);
                (
                    written
                        .strings()
                        .get(written.type_string(field.class))
                        .into_owned(),
                    written.strings().get(field.name).into_owned(),
                )
            });
        assert_eq!(read, Some((owner.to_string(), "c".to_string())), "{method}");
    }
}

#[test]
fn queries_skip_the_classes_a_bundle_extension_defines() {
    use reseam_patcher::context::ExtensionSet;

    const APP_STATE: &str = "Lcom/example/VideoState;";
    const EXT_STATE: &str = "Lapp/reseam/test/ext/VideoState;";

    let extension_dir = tempfile::tempdir().unwrap();
    let extension_path = extension_dir.path().join("ext.dex");
    fs::write(&extension_path, dex!("extension_shadow")).unwrap();

    let (_apk_dir, mut apk) =
        open_split_test_apk_with(&manifest_bytes("1.0-base", None), &[dex!("query_domain")]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let mut ctx = PatchContext::new(&mut apk);
    ctx.set_extensions(ExtensionSet::load(&[extension_path]).unwrap());
    run_one_patch(&mut ctx, &patches, "extension-shadow", "extensionShadow");

    assert_eq!(
        apk.read_component_entry(0, "assets/extension-shadow.txt")
            .unwrap(),
        Some(format!("{APP_STATE}|{EXT_STATE}").into_bytes()),
        "a plain query must find the app's class; includeExtensions() widens it to the bundle's"
    );
}

const SHARED_CLASS: &str = "Lapp/reseam/test/Shared;";

fn extension_files(dir: &Path, files: &[(&str, &[u8])]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|(name, bytes)| {
            let path = dir.join(format!("{name}.dex"));
            fs::write(&path, bytes).unwrap();
            path
        })
        .collect()
}

#[test]
fn extensions_share_a_class_they_define_identically() {
    use reseam_patcher::context::ExtensionSet;

    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(
        dir.path(),
        &[("a", dex!("shared_class_a")), ("b", dex!("shared_class_b"))],
    );
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("types")]);
    let mut ctx = PatchContext::new(&mut apk);
    ctx.set_extensions(ExtensionSet::load(&paths).unwrap());
    ctx.find_or_link_class("Lapp/reseam/test/Bee;").unwrap();
    ctx.find_or_link_class("Lapp/reseam/test/A;").unwrap();
    drop(ctx);

    let definitions = apk
        .dex()
        .iter()
        .filter(|dex| dex.find_class_index(SHARED_CLASS).is_some())
        .count();
    assert_eq!(definitions, 1, "both extensions must link the one copy");
}

#[test]
fn bundles_cannot_define_app_runtime_classes() {
    use reseam_dex::AccessFlags;
    use reseam_patcher::context::ExtensionSet;

    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    dex.create_class(
        "Lapp/reseam/runtime/Reseam;",
        AccessFlags::PUBLIC,
        Some("Ljava/lang/Object;"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(dir.path(), &[("runtime", &dex_bytes(&dex))]);
    let Err(error) = ExtensionSet::load(&paths) else {
        panic!("a bundle's copy of the app runtime must not load");
    };
    assert!(
        error.to_string().contains("Lapp/reseam/runtime/Reseam;"),
        "{error}"
    );
}

#[test]
fn extensions_defining_a_class_differently_conflict() {
    use reseam_patcher::context::ExtensionSet;

    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(
        dir.path(),
        &[
            ("a", dex!("shared_class_a")),
            ("conflict", dex!("shared_class_conflict")),
        ],
    );
    let Err(error) = ExtensionSet::load(&paths) else {
        panic!("differing definitions of {SHARED_CLASS} must not load");
    };
    assert!(
        error
            .to_string()
            .contains(&format!("{SHARED_CLASS} is defined differently")),
        "{error}"
    );
}

#[test]
fn a_new_attribute_binds_the_id_the_inflater_resolves_it_by() {
    const TARGET_PACKAGE: u32 = 0x0101_0021;
    const TARGET_CLASS: u32 = 0x0101_002f;

    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "xml-attribute-binding", "xmlAttributeBinding");

    let manifest = apk.base().manifest();
    let intent = manifest.find_element("intent").unwrap();
    let value = |res_id| {
        manifest
            .attribute(intent, res_id)
            .and_then(|attr| manifest.attribute_string(attr))
            .map(std::borrow::Cow::into_owned)
    };
    assert_eq!(value(TARGET_PACKAGE).as_deref(), Some("com.example.target"));
    assert_eq!(
        value(TARGET_CLASS).as_deref(),
        Some("com.example.target.Settings")
    );
}

fn inline_file_paths(table: &ResourceTable, pulse: &reseam_apk::AxmlDocument) -> [String; 2] {
    let root = pulse.root().unwrap();
    let target = pulse.find_element("target").unwrap();
    [(root, "drawable"), (target, "animation")].map(|(element, attribute)| {
        let value = pulse.attribute_named(element, attribute).unwrap().value();
        assert_eq!(value.kind, ResValue::REFERENCE);
        let (_, file) = table.values(value.data).next().unwrap().unwrap();
        assert_eq!(file.kind, ResValue::STRING);
        table.get_string(file.data).unwrap().unwrap().into_owned()
    })
}

fn assert_resource_values(table: &ResourceTable) {
    const WINDOW_BACKGROUND: u32 = 0x0101_0054;
    const TEXT_COLOR: u32 = 0x0101_0098;
    let icon = table
        .find_resource_id("drawable", "reseam_icon")
        .unwrap()
        .unwrap();
    for ((kind, name), path) in [
        (("drawable", "reseam_icon"), "res/reseam_icon.png"),
        (("mipmap", "reseam_launcher"), "res/reseam_launcher.png"),
    ] {
        assert_eq!(table.file_paths(kind, name).unwrap(), [path]);
    }
    let items = |name: &str| {
        table
            .complex_entries("style", name)
            .unwrap()
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
        .unwrap()
        .unwrap();
    assert_eq!(
        items("Theme.Reseam").first().map(|item| item.0),
        Some(app_attr)
    );
    let elements = table
        .complex_entries("array", "double_tap_lengths")
        .unwrap()
        .unwrap();
    assert!(
        elements[..7]
            .iter()
            .all(|entry| entry.value.kind == ResValue::STRING)
    );
    assert_eq!(
        elements[..7]
            .iter()
            .map(|entry| table
                .get_string(entry.value.data)
                .unwrap()
                .unwrap()
                .into_owned())
            .collect::<Vec<_>>(),
        [
            "5",
            "10",
            "15",
            "20",
            &format!("@0x{icon:08x}"),
            "true",
            "@string/missing"
        ]
    );
    let scalars: Vec<_> = elements[7..].iter().map(|entry| entry.value).collect();
    assert_eq!(
        scalars,
        [
            ResValue::new(ResValue::FLOAT, 0x8000_0000),
            ResValue::new(ResValue::DIMENSION, 0x0000_2101),
            ResValue::new(ResValue::INT_BOOLEAN, 1),
            ResValue::reference(icon)
        ]
    );
}

#[test]
fn a_typed_file_resource_a_style_and_an_array_land_where_the_loader_reads_them() {
    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "resource-entries", "resourceEntries");

    assert_eq!(
        apk.read_component_entry(0, "assets/resource-entries.txt")
            .unwrap(),
        Some(format!("{LAYOUT_PATH}|5,10").into_bytes()),
        "the layout resolved to its obfuscated path and the array read back as text"
    );
    assert!(
        apk.read_component_entry(0, "res/reseam_icon.png")
            .unwrap()
            .is_some()
    );
    let pulse_bytes = entry(&mut apk, 0, "res/drawable/reseam_pulse.xml").unwrap();
    let table = apk.component_mut(0).unwrap().resources().unwrap().unwrap();

    assert_resource_values(table);
    let pulse = reseam_apk::AxmlDocument::parse(&pulse_bytes).unwrap();
    let paths = inline_file_paths(table, &pulse);
    for (path, element) in paths.iter().zip(["vector", "objectAnimator"]) {
        let bytes = entry(&mut apk, 0, path).unwrap();
        let document = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
        assert!(document.find_element(element).is_some());
    }
    let bytes = entry(&mut apk, 0, "res/split.xml").unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert_eq!(
        doc.element_name(doc.root().unwrap()).as_deref(),
        Some("base")
    );
    assert!(doc.find_element("split").is_none());
    let bytes = entry(&mut apk, 1, "res/split.xml").unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert!(doc.find_element("split").is_some());
}

#[test]
fn an_adopted_subtree_carries_the_ids_the_inflater_resolves_it_by() {
    const RES_AUTO: &str = "http://schemas.android.com/apk/res-auto";
    const ANDROID_ID: u32 = 0x0101_00d0;
    const ANDROID_SRC: u32 = 0x0101_0119;

    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "xml-graft", "xmlGraft");

    let layout = entry(&mut apk, 0, LAYOUT_PATH).unwrap();
    let layout = reseam_apk::AxmlDocument::parse(&layout).unwrap();
    let table = apk.component_mut(0).unwrap().resources().unwrap().unwrap();
    let app_attr = table
        .find_resource_id("attr", "layout_constraintRight_toLeftOf")
        .unwrap()
        .unwrap();
    let controls = table.find_resource_id("id", "controls").unwrap().unwrap();
    let fullscreen = table
        .find_resource_id("id", "fullscreen_button")
        .unwrap()
        .unwrap();
    let icon = table
        .find_resource_id("drawable", "reseam_icon")
        .unwrap()
        .unwrap();

    let container = layout.find_element("FrameLayout").unwrap();
    let res_auto = layout.namespace_index(RES_AUTO).unwrap();
    let constraint = layout.attribute(container, app_attr).unwrap();
    assert_eq!(constraint.value(), ResValue::reference(fullscreen));
    assert_eq!(
        constraint.namespace(),
        Some(res_auto),
        "the source prefix was yt, the target declares app, and both mean res-auto"
    );
    assert_eq!(
        layout
            .attribute(container, ANDROID_ID)
            .map(|attr| attr.value().kind),
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

    let image = (0..layout.events().len())
        .filter(|&i| layout.element_name(i).as_deref() == Some("ImageView"))
        .collect::<Vec<_>>();
    assert_eq!(image.len(), 2);
    assert_eq!(
        layout
            .attribute(image[1], ANDROID_SRC)
            .map(AxmlAttribute::value),
        Some(ResValue::reference(icon)),
        "the fragment compiled against the drawable the patch had just registered"
    );
    assert_eq!(
        layout
            .attribute(image[0], app_attr)
            .map(AxmlAttribute::value),
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
            .map(AxmlAttribute::value),
        Some(ResValue::reference(controls))
    );
}

#[test]
fn a_query_can_require_instruction_order_and_a_predicate_of_its_own() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("ordered_queries")]);
    apply_fixture(
        &mut apk,
        "instruction-shape-queries",
        "instructionShapeQueries",
    );

    assert_eq!(
        apk.read_component_entry(0, "assets/instruction-shape.txt")
            .unwrap(),
        Some(b"alpha|gamma|shown|Lcom/example/Binder;".to_vec()),
    );
}

#[test]
fn a_class_reaches_the_methods_its_base_declares() {
    const BASE: &str = "Lcom/example/PreferenceBase;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("inherited_app"), dex!("inherited_base")]);
    apply_fixture(&mut apk, "inherited-methods", "inheritedMethods");

    assert_eq!(
        apk.read_component_entry(0, "assets/inherited-methods.txt")
            .unwrap(),
        Some(format!("{BASE}|{BASE}").into_bytes()),
        "inClass(inherited) and a call site's callee both resolve to the base declaration",
    );
}

fn patched_code(apk: &ApkFile, owner: &str, name: &str) -> reseam_dex::CodeItem {
    read_method(&written_dex(apk, 0), owner, name)
        .unwrap()
        .code
        .map(std::sync::Arc::unwrap_or_clone)
        .unwrap()
}

fn read_method(dex: &DexFile, owner: &str, name: &str) -> Option<reseam_dex::EncodedMethod> {
    let class = dex.find_class_index(owner)?;
    let name = dex.strings().find(name)?;
    let hit = dex
        .scan_methods_find(&reseam_dex::RefQuery::default(), |view| {
            Ok(
                (view.class_idx == class && dex.method_id(view.method).name == name)
                    .then(|| view.hit()),
            )
        })
        .unwrap()?;
    dex.decode_method_at(hit.class_idx, hit.method_pos, hit.kind)
        .unwrap()
}

#[test]
fn emissions_at_one_point_land_in_order_and_a_replaced_body_loses_it() {
    const OWNER: &str = "Lcom/example/AnchorHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("point_order")]);
    apply_fixture(&mut apk, "point-anchors", "pointAnchors");

    let twice = patched_code(&apk, OWNER, "twice");
    assert_eq!(
        hook_calls(&twice, &[]),
        (vec![vec![40], vec![30], vec![10], vec![20], vec![50]], None)
    );
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "replaced"), &[]),
        (vec![], None)
    );
}

#[test]
fn a_reserved_local_carries_a_value_between_blocks() {
    const OWNER: &str = "Lcom/example/LocalHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("reserved_local")]);
    apply_fixture(&mut apk, "reserved-locals", "reservedLocals");

    let carry = patched_code(&apk, OWNER, "carry");
    assert_eq!(
        hook_calls(&carry, &[5]),
        (vec![vec![0], vec![1, 2], vec![77], vec![1, 1]], Some(9))
    );
}

#[test]
fn an_argument_capture_names_the_register_the_invoke_passes() {
    const OWNER: &str = "Lcom/example/ArgHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("capture_arguments")]);
    apply_fixture(&mut apk, "captured-arguments", "capturedArguments");

    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "run"), &[201, 301]),
        (vec![vec![201, 301, 5], vec![201, 301, 5]], None)
    );
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "wide"), &[101, 102, 103]),
        (vec![vec![101, 102, 103], vec![1, 101, 102, 103]], None)
    );
}

#[test]
fn a_writer_is_the_one_instruction_that_defined_an_argument() {
    const OWNER: &str = "Lcom/example/WriterHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("register_writers")]);
    apply_fixture(&mut apk, "register-writers", "registerWriters");

    let report = entry(&mut apk, 0, "assets/register-writers.txt").unwrap();
    assert_eq!(String::from_utf8(report).unwrap(), "true\ntrue\ntrue");
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "single"), &[6]),
        (vec![vec![99], vec![4]], None)
    );
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "copied"), &[6]),
        (vec![vec![98], vec![7]], None)
    );
}

#[test]
fn point_queries_and_redirects_share_tracked_control_flow() {
    use reseam_dex::Instruction::*;
    const OWNER: &str = "Lcom/example/PointQueryHost;";

    let (_dir, mut apk) = open_dex_apk(&[dex!("point_queries")]);
    apk.add_dex(reseam_dex::parse(dex!("extension_caller"), ParseOptions::default()).unwrap());
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "point-queries",
        "pointQueries",
    );
    let report = entry(&mut apk, 0, "assets/point-queries.txt").unwrap();
    assert_eq!(
        String::from_utf8(report).unwrap(),
        "copied:1\nsame:1\nconflict:0\npassed:0\nwide:1\nwideCopied:1\nwideBroken:0\nexception:1\nduplicates:2\noverwritten:1\nunknown:0"
    );
    let code = patched_code(&apk, OWNER, "duplicates");
    assert_eq!(
        code.instructions()
            .iter()
            .filter(|i| matches!(i, InvokeStatic { .. } | InvokeStaticRange { .. }))
            .count(),
        4
    );
    assert_eq!(hook_calls(&code, &[7]), (vec![vec![4]; 4], None));
    run_one_patch(
        &mut PatchContext::new(&mut apk),
        &patches,
        "point-redirects",
        "pointRedirects",
    );
    let wide = patched_code(&apk, OWNER, "wide");
    assert!(matches!(
        wide.instructions()
            .iter()
            .find(|insn| insn.method_ref().is_some())
            .unwrap(),
        InvokeStatic { .. } | InvokeStaticRange { .. }
    ));
}

#[test]
fn string_prefix_queries_use_literal_prefixes_and_exclude_extensions() {
    let (_dir, mut apk) = open_dex_apk(&[dex!("prefix_queries")]);
    apk.add_dex(reseam_dex::parse(dex!("extension_routes"), ParseOptions::default()).unwrap());
    apply_fixture(&mut apk, "string-prefix-queries", "stringPrefixQueries");
}

#[test]
fn high16_literals_are_consistent_in_queries_and_mutations() {
    use reseam_dex::Instruction::*;

    let owner = "Lcom/example/High16;";

    let (_dir, mut apk) = open_dex_apk(&[dex!("high_literals")]);
    apply_fixture(&mut apk, "high16-literals", "high16Literals");
    assert!(matches!(
        patched_code(&apk, owner, "positive").instructions()[0],
        ConstHigh16 { value: 2048, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "negative").instructions()[0],
        ConstHigh16 { value: -2, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "wide").instructions()[0],
        ConstWideHigh16 {
            value: i16::MIN,
            ..
        }
    ));
}

#[test]
fn hierarchy_queries_follow_subclasses_across_dex_files_and_direct_interfaces() {
    use reseam_dex::AccessFlags;

    // PreferenceFragment, in the first DEX, extends PreferenceBase from the second.
    let (_dir, mut apk) = open_dex_apk(&[dex!("inherited_app"), dex!("inherited_base")]);
    let mut ctx = PatchContext::new(&mut apk);
    let base = ctx.dex_file_mut(1).unwrap();
    let object = Some("Ljava/lang/Object;");
    let fragment = Some("Lcom/example/PreferenceFragment;");
    base.create_class("Lcom/example/DeepFragment;", AccessFlags::PUBLIC, fragment)
        .unwrap();
    base.create_class("Lcom/example/Listener;", AccessFlags::INTERFACE, object)
        .unwrap();
    let listener = base.intern_type("Lcom/example/Listener;").unwrap();
    let implementer = base
        .create_class("Lcom/example/Impl;", AccessFlags::PUBLIC, object)
        .unwrap();
    base.class_mut(implementer)
        .unwrap()
        .interfaces
        .push(listener);
    base.create_class(
        "Lcom/example/ImplChild;",
        AccessFlags::PUBLIC,
        Some("Lcom/example/Impl;"),
    )
    .unwrap();

    let names = |found: Vec<reseam_patcher::context::ClassLocation>| {
        let mut names: Vec<String> = found
            .iter()
            .map(|location| {
                let dex = ctx.dex_file(location.dex_idx).unwrap();
                let class_type = dex.classes().header(location.class_idx).class_type;
                dex.type_descriptor(class_type).into_owned()
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        names(
            ctx.find_classes_extending("Lcom/example/PreferenceBase;")
                .unwrap()
        ),
        [
            "Lcom/example/DeepFragment;",
            "Lcom/example/PreferenceFragment;"
        ],
    );
    assert_eq!(
        names(
            ctx.find_classes_implementing("Lcom/example/Listener;")
                .unwrap()
        ),
        ["Lcom/example/Impl;"],
        "a subclass inherits the interface without declaring it",
    );
    assert!(ctx.find_classes_extending("LAbsent;").unwrap().is_empty());
}

#[test]
fn instance_field_queries_read_lazy_fields_and_observe_edits() {
    use reseam_dex::{AccessFlags, EncodedField};

    let (_dir, mut apk) = open_dex_apk(&[dex!("instance_fields")]);
    let mut ctx = PatchContext::new(&mut apk);
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    let matches = ctx
        .find_classes_with_instance_field("Ljava/lang/String;")
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    assert!(
        ctx.find_classes_with_instance_field("LAbsent;")
            .unwrap()
            .is_empty()
    );
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
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn settings_pages_merge_selected_contributions_and_keep_nested_ancestors() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
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
        let (_dir, mut apk) = open_settings_test_apk();
        let selection = PatchSelection {
            enable: [format!("runtime-test-bundle/app.reseam.test.{patch}")].into(),
            ..Default::default()
        };
        let results = apply(&mut PatchContext::new(&mut apk), &patches, &selection).unwrap();
        assert!(
            results
                .iter()
                .all(|r| !matches!(r.status, PatchStatus::Failed { .. }))
        );
        let bytes = entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap();
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
        assert_eq!(child["title"]["table"], "TestStrings");
        assert_eq!(child["title"]["name"], "page_quality");
        let table = entry(&mut apk, 0, SETTINGS_STRINGS).unwrap();
        let table: serde_json::Value = serde_json::from_slice(&table).unwrap();
        assert_eq!(table["page_quality"], "Quality");
        assert!(entry(&mut apk, 0, RUNTIME_STRINGS).is_some());
        let sections = schema["sections"].as_array().unwrap();
        assert!(sections[0]["page"].is_null());
        assert_eq!(sections[0]["settings"][0]["key"], "root.enabled");
        assert_eq!(sections[0]["settings"][1]["multiline"], true);
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

#[test]
fn settings_schemas_carry_the_bundle_license_and_normalized_svg_icons() {
    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(
        &mut apk,
        "Settings navigation first",
        "navigationFirstPatch",
    );
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    assert_eq!(schema["license"], "GPL-3.0");
    let about = &schema["about"];
    assert_eq!(about["links"][0]["label"]["text"], "Forum");
    assert_eq!(about["links"][0]["url"], "https://example.com/forum");
    assert_eq!(
        about["links"][0]["icon"]["paths"][0]["data"],
        "M 9 18 L 15 12 L 9 6"
    );
    assert_eq!(about["actions"][0]["label"]["name"], "page_media");
    assert_eq!(about["actions"][0]["url"], "https://example.com/donate");
    let media = schema["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["id"] == "media")
        .unwrap();
    let chevron = &media["icon"]["paths"][0];
    assert_eq!(chevron["data"], "M 9 18 L 15 12 L 9 6");
    assert!(
        chevron["fill"].is_null(),
        "fill=\"none\" on <svg> is inherited"
    );
    assert!(
        chevron["stroke"]["paint"].is_null(),
        "currentColor takes the tint"
    );
    assert_eq!(chevron["stroke"]["cap"], "round");
    let dot = &schema["sections"][0]["settings"][0]["icon"];
    assert_eq!(dot["width"], 4.0);
    let data = dot["paths"][0]["data"].as_str().unwrap();
    assert!(
        data.starts_with("M 0 2 C ") && data.ends_with(" Z"),
        "the circle becomes cubics, shifted to the viewBox origin: {data}"
    );
    assert_eq!(dot["paths"][0]["fill"]["paint"], "#FFFF0000");
}

#[test]
fn settings_hosts_start_the_app_runtime_and_gate_on_the_bundles_own_settings() {
    use reseam_dex::Instruction::ConstString;

    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings gated entry", "gatedEntry");
    let written = written_dex(&apk, 0);
    assert_eq!(
        invoked_by(&written, "Lapp/reseam/AppEntry;", "onCreate"),
        [
            "Lapp/reseam/runtime/Reseam;->init",
            "Lapp/reseam/runtime/Reseam;->isEnabled",
            "Lcom/example/Observer;->started",
        ],
        "the app runtime starts before any entry code reads a setting"
    );
    let code = read_method(&written, "Lapp/reseam/AppEntry;", "onCreate")
        .unwrap()
        .code
        .unwrap();
    assert!(
        code.instructions().iter().any(|instruction| matches!(
            instruction,
            ConstString { string, .. } if written.strings().get(*string) == "runtime-test-bundle"
        )),
        "gates name the bundle whose settings they read"
    );
    assert!(
        apk.dex().iter().any(|dex| dex
            .find_class_index("Lapp/reseam/runtime/Reseam;")
            .is_some()),
        "the engine links its app runtime"
    );
}

#[test]
fn emitted_equality_preserves_numeric_values_and_reference_identity() {
    let owner = "Lcom/example/ComparisonHost;";

    let (_directory, mut apk) = open_dex_apk(&[dex!("comparisons")]);
    apply_fixture(&mut apk, "typed-comparisons", "typedComparisons");
    let wide_words = |bits: u64| [i64::from(bits as u32), i64::from((bits >> 32) as u32)];
    for (kind, cases) in [
        (
            "long",
            vec![
                ((0_u64, 0_u64), true),
                ((1, 1 << 32), false),
                ((i64::MIN as u64, i64::MAX as u64), false),
                ((u64::MAX, u64::MAX), true),
            ],
        ),
        (
            "float",
            vec![
                ((0, u64::from((-0.0_f32).to_bits())), true),
                (
                    (u64::from(f32::NAN.to_bits()), u64::from(f32::NAN.to_bits())),
                    false,
                ),
                (
                    (u64::from(1.0_f32.to_bits()), u64::from(2.0_f32.to_bits())),
                    false,
                ),
            ],
        ),
        (
            "double",
            vec![
                ((0, (-0.0_f64).to_bits()), true),
                ((f64::NAN.to_bits(), f64::NAN.to_bits()), false),
                ((1.0_f64.to_bits(), 2.0_f64.to_bits()), false),
            ],
        ),
        (
            "int",
            vec![((0, 0), true), ((42, 42), true), ((1, 2), false)],
        ),
        (
            "reference",
            vec![
                ((0, 0), true),
                ((101, 101), true),
                ((101, 202), false),
                ((0, 101), false),
            ],
        ),
    ] {
        for ((a, b), equal) in cases {
            let incoming = if kind == "long" || kind == "double" {
                [wide_words(a), wide_words(b)].concat()
            } else {
                vec![a as i64, b as i64]
            };
            for (suffix, expected) in [("Equal", equal), ("NotEqual", !equal)] {
                let code = patched_code(&apk, owner, &format!("{kind}{suffix}"));
                assert_eq!(
                    hook_calls(&code, &incoming),
                    (vec![], Some(i64::from(expected))),
                    "{kind}{suffix}({a}, {b})"
                );
            }
        }
    }
}

#[test]
fn emitted_arguments_accept_unresolved_hierarchies_and_reject_proven_mismatches() {
    let owner = "Lcom/example/TypeHost;";

    let (_directory, mut apk) = open_dex_apk(&[dex!("types")]);
    apply_fixture(&mut apk, "incomplete-hierarchies", "incompleteHierarchies");
    for name in [
        "application",
        "image",
        "appView",
        "libraryActivity",
        "missingSource",
        "missingTarget",
        "knownSubclass",
    ] {
        assert_eq!(
            hook_calls(&patched_code(&apk, owner, name), &[123]),
            (vec![vec![123]], None),
            "{name}"
        );
    }
    for name in [
        "unrelated",
        "primitiveReference",
        "wideNarrow",
        "arrayElements",
    ] {
        let code = patched_code(&apk, owner, name);
        assert_eq!(
            hook_calls(&code, &[123, 456][..usize::from(code.ins_size())]),
            (vec![], None),
            "{name}"
        );
    }
    assert_eq!(
        apk.read_component_entry(0, "assets/type-validation.txt")
            .unwrap(),
        Some(b"true|true|true|true".to_vec())
    );
}

#[test]
fn emitted_constants_preserve_values_across_encoding_widths_and_branches() {
    let owner = "Lcom/example/ConstantHost;";

    let cases = [
        ("small", 7),
        ("shortMax", 32767),
        ("shortMin", -32768),
        ("positiveWide", 32768),
        ("negativeWide", -32769),
        ("positiveHigh", 65536),
        ("negativeHigh", -65536),
        ("color", -16_777_216),
        ("minimum", i32::MIN),
        ("maximum", i32::MAX),
    ];

    let (_directory, mut apk) = open_dex_apk(&[dex!("constants")]);
    apply_fixture(&mut apk, "constant-forms", "constantForms");
    for (name, value) in cases {
        let code = patched_code(&apk, owner, name);
        for (condition, expected) in [(0, 0), (1, value)] {
            assert_eq!(
                hook_calls(&code, &[condition]),
                (vec![], Some(i64::from(expected))),
                "{name}({condition})"
            );
        }
    }
}
