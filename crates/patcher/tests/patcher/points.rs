// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{entry, open_dex_apk};
use crate::common::bundles::{apply, apply_fixture, loaded_test_bundle, run_one_patch};
use crate::common::code::{hook_calls, patched_code};
use crate::common::dex;
use reseam_apk::ApkFile;
use reseam_dex::ParseOptions;
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{PatchSelection, PatchStatus};

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
fn a_cast_leaves_the_parameter_it_casts_unchanged() {
    use reseam_dex::Instruction;

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("instance_branch")]);
    apply_fixture(&mut apk, "cast-parameter", "castParameter");
    let code = patched_code(&apk, "Lcom/example/SkipHost;", "typed");
    let incoming = code.registers_size() - code.ins_size();
    let casts: Vec<u16> = code
        .instructions()
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::CheckCast { ref_, .. } => Some(u16::from(*ref_)),
            _ => None,
        })
        .collect();
    assert!(!casts.is_empty());
    assert!(casts.iter().all(|&register| register < incoming));
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
    let results = apply(
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
        &patches,
        &selection,
    )
    .unwrap();
    let result = results
        .iter()
        .find(|r| r.patch.id.ends_with(id))
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
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
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
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
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
