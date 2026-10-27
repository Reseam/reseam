// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{SETTINGS_SCHEMA, entry, open_settings_test_apk, open_split_test_apk};
use crate::common::bundles::{apply, load_all, loaded_test_bundle, write_bundle_reseam};
use reseam_apk::ApkFile;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{PatchSelection, PatchStatus};
use reseam_patcher::options::OptionValue;
use reseam_patcher::{Patch, PatchPreset, PatchRef};

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
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());

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
        .map(|result| (result.patch.to_string(), &result.status))
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
        let result = apply(
            &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
            &patches,
            &selection,
        );
        match expected {
            SelectionOutcome::MissingBundle => assert!(matches!(result,
                Err(PatcherError::MissingBundle(unresolved))
                    if unresolved.patch.to_string() == "runtime-test-bundle/app.reseam.test.needsOtherBundle"
                        && unresolved.dependency.to_string() == "other-bundle/app.reseam.other.helper")),
            SelectionOutcome::MissingOption => assert!(matches!(result,
                Err(PatcherError::MissingRequiredOption { key, .. }) if key == "token")),
            SelectionOutcome::NotRun(id) => {
                assert!(
                    !result
                        .unwrap()
                        .iter()
                        .any(|item| item.patch.id.ends_with(id))
                );
            }
            SelectionOutcome::Applied(ids) => {
                let results = result.unwrap();
                let applied: Vec<_> = results
                    .iter()
                    .filter(|item| item.status == PatchStatus::Applied)
                    .map(|item| item.patch.id.rsplit('.').next().unwrap())
                    .collect();
                assert_eq!(applied, ids);
                if let Some(helper) = results.iter().find(|item| {
                    item.patch.id.ends_with("internalHelper") && item.status == PatchStatus::Applied
                }) {
                    assert!(helper.hidden);
                    assert_eq!(
                        helper.required_by,
                        [
                            PatchRef::parse("runtime-test-bundle/app.reseam.test.usesInternal")
                                .unwrap()
                        ]
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
    let results = apply(
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
        &patches,
        &selection,
    )
    .unwrap();
    let applied: Vec<_> = results
        .iter()
        .filter(|r| r.status == PatchStatus::Applied)
        .map(|r| r.patch.to_string())
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
        let results = apply(
            &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
            &patches,
            &selection,
        )
        .unwrap();
        let status = |id: &str| {
            results
                .iter()
                .find(|result| {
                    result.patch.to_string() == format!("runtime-test-bundle/app.reseam.test.{id}")
                })
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
