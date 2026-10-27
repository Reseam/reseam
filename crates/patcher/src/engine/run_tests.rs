// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::context::ExtensionSet;
use crate::engine::{Delivery, PatchSelection};
use crate::error::PatcherError;
use crate::patch::{
    ApkVersion, Compatibility, CompatiblePackage, PatchPreset, PatchRef, PatchSpec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Applied,
    Skipped,
    Unmountable,
    Failed,
}

use Status::{Applied as A, Failed as F, Skipped as S};

impl From<&PatchStatus> for Status {
    fn from(status: &PatchStatus) -> Self {
        match status {
            PatchStatus::Applied => Self::Applied,
            PatchStatus::Skipped { .. } => Self::Skipped,
            PatchStatus::Unmountable { .. } => Self::Unmountable,
            PatchStatus::Failed { .. } => Self::Failed,
        }
    }
}

fn declared(bundle: &str, id: &str, dependencies: &[&str]) -> PatchSpec {
    PatchSpec {
        reference: PatchRef {
            bundle: bundle.into(),
            id: id.into(),
        },
        name: id.into(),
        hidden: false,
        description: String::new(),
        enabled_by_default: true,
        dependencies: dependencies
            .iter()
            .map(|d| PatchRef::parse(d).unwrap())
            .collect(),
        compatibility: [CompatiblePackage {
            package: "com.example".into(),
            versions: Vec::new(),
            version_codes: Vec::new(),
        }]
        .into_iter()
        .collect(),
        options: Vec::new(),
        rewrites_output: false,
    }
}

fn validate(
    specs: &[&PatchSpec],
    selection: &PatchSelection,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    let version = ApkVersion {
        name: version,
        code: None,
    };
    let plan = ResolvedPlan::resolve(specs, selection, Delivery::Install, package, version)?;
    Ok(validate_patches(specs, &plan))
}

fn statuses(
    declarations: &[&PatchSpec],
    selection: &PatchSelection,
    package: &str,
    version: &str,
) -> Vec<(String, Status)> {
    validate(declarations, selection, Some(package), Some(version))
        .unwrap()
        .iter()
        .map(|r| (r.patch.to_string(), Status::from(&r.status)))
        .collect()
}

fn covered(expected: &[(&str, Status)]) -> Vec<(String, Status)> {
    expected
        .iter()
        .map(|&(id, status)| (format!("bundle/{id}"), status))
        .collect()
}

#[test]
fn selection_presets_and_compatibility() {
    let own = declared("bundle", "own", &[]);
    let mut pinned = declared("bundle", "pinned", &[]);
    pinned.compatibility = [CompatiblePackage {
        package: "com.example".into(),
        versions: vec!["1.0".into()],
        version_codes: Vec::new(),
    }]
    .into_iter()
    .collect();
    let mut optional = declared("bundle", "optional", &[]);
    optional.enabled_by_default = false;
    let mut universal = declared("bundle", "universal", &[]);
    universal.compatibility = Compatibility::Universal;
    universal.enabled_by_default = false;
    let mut opted_in = declared("bundle", "opted-in", &[]);
    opted_in.compatibility = Compatibility::Universal;
    let patches = [&own, &pinned, &optional, &universal, &opted_in];
    for (selection, (package, expected)) in [
        (
            PatchSelection::default(),
            (
                "com.example",
                &[("own", A), ("pinned", S), ("opted-in", A)][..],
            ),
        ),
        (
            PatchSelection {
                ignore_versions: true,
                ..Default::default()
            },
            ("com.example", &[("own", A), ("pinned", A), ("opted-in", A)]),
        ),
        (
            PatchSelection {
                enable: vec!["universal".into()],
                ..Default::default()
            },
            (
                "com.example",
                &[("own", A), ("pinned", S), ("universal", A), ("opted-in", A)],
            ),
        ),
        (
            PatchSelection {
                preset: PatchPreset::All,
                disable: vec!["own".into()],
                ..Default::default()
            },
            (
                "com.example",
                &[("own", S), ("pinned", S), ("optional", A), ("opted-in", A)],
            ),
        ),
        (
            PatchSelection {
                preset: PatchPreset::None,
                ..Default::default()
            },
            ("com.example", &[]),
        ),
        (PatchSelection::default(), ("com.other", &[("opted-in", A)])),
    ] {
        assert_eq!(
            statuses(&patches, &selection, package, "2.0"),
            covered(expected)
        );
    }
}

#[test]
fn dependency_selection_stops_at_unavailable_or_incompatible_nodes() {
    let mut helper = declared("bundle", "helper", &[]);
    helper.hidden = true;
    helper.enabled_by_default = false;
    helper.compatibility = Compatibility::Universal;
    let mut middle = declared("bundle", "middle", &["bundle/helper"]);
    middle.enabled_by_default = false;
    let root = declared("bundle", "root", &["bundle/middle", "bundle/helper"]);
    for (selection, (package, expected)) in [
        (
            PatchSelection::default(),
            (
                "com.example",
                &[("helper", A), ("middle", A), ("root", A)][..],
            ),
        ),
        (PatchSelection::default(), ("com.other", &[])),
        (
            PatchSelection {
                disable: vec!["root".into()],
                ..Default::default()
            },
            ("com.example", &[("root", S)]),
        ),
        (
            PatchSelection {
                disable: vec!["middle".into()],
                ..Default::default()
            },
            ("com.example", &[("middle", S), ("root", S)]),
        ),
    ] {
        assert_eq!(
            statuses(&[&helper, &middle, &root], &selection, package, "2.0"),
            covered(expected)
        );
    }
    middle.compatibility = [CompatiblePackage {
        package: "com.other".into(),
        versions: Vec::new(),
        version_codes: Vec::new(),
    }]
    .into_iter()
    .collect();
    assert_eq!(
        statuses(
            &[&helper, &middle, &root],
            &PatchSelection::default(),
            "com.example",
            "2.0"
        ),
        covered(&[("middle", S), ("root", S)])
    );
    middle
        .dependencies
        .push(PatchRef::parse("missing/helper").unwrap());
    assert_eq!(
        statuses(
            &[&helper, &middle, &root],
            &PatchSelection::default(),
            "com.example",
            "2.0"
        ),
        covered(&[("middle", S), ("root", S)])
    );
}

#[test]
fn reference_selection_disambiguates_bundles() {
    let official = declared("official", "helper", &[]);
    let fork = declared("fork", "helper", &[]);
    let consumer = declared("fork", "consumer", &["official/helper"]);
    let specs = [official, fork, consumer];
    let specs: Vec<_> = specs.iter().collect();
    let selection = PatchSelection {
        preset: PatchPreset::None,
        enable: vec!["consumer".into()],
        ..Default::default()
    };
    let results = validate(&specs, &selection, Some("com.example"), None).unwrap();
    assert_eq!(
        results
            .iter()
            .filter(|r| r.status == PatchStatus::Applied)
            .map(|r| r.patch.to_string())
            .collect::<Vec<_>>(),
        ["official/helper", "fork/consumer"]
    );
    let selection = PatchSelection {
        enable: vec!["helper".into()],
        ..selection
    };
    assert!(matches!(
        validate(&specs, &selection, Some("com.example"), None),
        Err(PatcherError::InvalidSelection(_))
    ));
}

#[test]
fn explicitly_selected_missing_dependencies_are_errors() {
    let consumer = declared("fork", "consumer", &["official/helper"]);
    let other = declared("official", "other", &[]);
    for specs in [
        vec![consumer.clone()],
        vec![consumer.clone(), other.clone()],
    ] {
        let specs: Vec<_> = specs.iter().collect();
        let results = validate(
            &specs,
            &PatchSelection::default(),
            Some("com.example"),
            None,
        )
        .unwrap();
        assert_eq!(Status::from(&results[0].status), Status::Skipped);
        let selection = PatchSelection {
            enable: vec!["consumer".into()],
            ..Default::default()
        };
        assert!(matches!(
            validate(&specs, &selection, Some("com.example"), None),
            Err(PatcherError::MissingBundle { .. } | PatcherError::MissingDependency { .. })
        ));
    }
}

#[path = "run_tests/finalization.rs"]
mod finalization;
#[path = "run_tests/rollback.rs"]
mod rollback;
