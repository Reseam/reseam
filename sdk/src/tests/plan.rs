// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use reseam_model::{
    AppIdentity, Compatibility, CompatiblePackage, Incompatibility, InstallMethod,
    OptionDeclaration, OptionIssue, OptionProblem, OptionType, OptionValue, PatchPreset,
    PatchSelection, PatchSpec, PlanRequest, PlannedPatch, PlannedRun,
};

use super::{metadata, reference, spec};
use crate::plan;

fn planned(
    patches: Vec<PatchSpec>,
    selection: PatchSelection,
    install_method: InstallMethod,
) -> (HashMap<String, PlannedPatch>, Vec<String>) {
    let plan = plan(&PlanRequest {
        patches: patches
            .into_iter()
            .map(|spec| metadata(spec, "fingerprint"))
            .collect(),
        app: AppIdentity {
            package_name: Some("com.example".to_owned()),
            version_name: Some("2.0".to_owned()),
            version_code: None,
        },
        selection,
        install_method,
    })
    .unwrap();
    let patches = plan
        .patches
        .into_iter()
        .map(|patch| (patch.patch.id.clone(), patch))
        .collect();
    (patches, plan.unknown)
}

#[test]
fn a_plan_shows_what_the_selection_runs_for_the_target() {
    let mut hidden = spec("hidden", Some(&[]));
    hidden.hidden = true;
    let mut needs_hidden = spec("needs_hidden", Some(&[]));
    needs_hidden.dependencies = vec![reference("hidden")];
    let mut optional = spec("optional", Some(&[]));
    optional.enabled_by_default = false;
    let mut other_app = spec("other_app", None);
    other_app.compatibility = Compatibility::Packages {
        packages: vec![CompatiblePackage {
            package: "com.other".to_owned(),
            versions: Vec::new(),
            version_codes: Vec::new(),
        }],
    };
    let (patches, unknown) = planned(
        vec![
            spec("old_version", Some(&["1.0"])),
            hidden,
            needs_hidden,
            optional,
            other_app,
            spec("universal", None),
        ],
        PatchSelection::default(),
        InstallMethod::Install,
    );
    assert!(unknown.is_empty());
    assert!(!patches.contains_key("other_app"));
    assert_eq!(patches["needs_hidden"].run, PlannedRun::Runs);
    assert_eq!(patches["hidden"].run, PlannedRun::Runs);
    assert_eq!(
        patches["hidden"].required_by,
        vec![reference("needs_hidden")]
    );
    assert!(!patches["hidden"].selected);
    assert_eq!(patches["optional"].run, PlannedRun::Off);
    assert_eq!(patches["optional"].presets, vec![PatchPreset::All]);
    assert_eq!(patches["universal"].run, PlannedRun::Runs);
    assert!(matches!(
        patches["old_version"].incompatibility,
        Some(Incompatibility::Version { .. })
    ));
    assert!(matches!(
        patches["old_version"].run,
        PlannedRun::Skipped { .. }
    ));
}

#[test]
fn a_plan_reports_what_a_run_would_reject_per_patch() {
    let mut configurable = spec("configurable", Some(&[]));
    configurable.options = vec![OptionDeclaration {
        key: "name".to_owned(),
        title: "Name".to_owned(),
        description: String::new(),
        option_type: OptionType::String,
        default_value: None,
        valid_values: None,
        required: true,
    }];
    let mut rewrites = spec("rewrites", Some(&[]));
    rewrites.rewrites_output = true;
    let (patches, unknown) = planned(
        vec![configurable, rewrites, spec("turned_off", Some(&[]))],
        PatchSelection {
            enable: vec!["bundle/removed".to_owned()],
            disable: vec!["bundle/turned_off".to_owned()],
            ..PatchSelection::default()
        },
        InstallMethod::Mount,
    );
    assert_eq!(unknown, vec!["bundle/removed".to_owned()]);
    assert_eq!(
        patches["configurable"].option_problems,
        vec![OptionIssue {
            key: "name".to_owned(),
            problem: OptionProblem::Missing,
        }]
    );
    assert!(matches!(
        patches["rewrites"].run,
        PlannedRun::Unmountable { .. }
    ));
    assert_eq!(patches["turned_off"].run, PlannedRun::Off);
    assert!(!patches["turned_off"].selected);
}

#[test]
fn a_plan_ignores_options_for_a_patch_it_does_not_run() {
    let (patches, unknown) = planned(
        vec![spec("turned_off", Some(&[]))],
        PatchSelection {
            disable: vec!["bundle/turned_off".to_owned()],
            options: HashMap::from([(
                "bundle/turned_off".to_owned(),
                HashMap::from([("name".to_owned(), OptionValue::Text("kept".to_owned()))]),
            )]),
            ..PatchSelection::default()
        },
        InstallMethod::Install,
    );
    assert!(unknown.is_empty());
    assert_eq!(patches["turned_off"].run, PlannedRun::Off);
    assert!(patches["turned_off"].option_problems.is_empty());
}
