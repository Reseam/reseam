// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use super::*;
use crate::error::PatcherError;
use crate::patch::{Compatibility, CompatiblePackage, PatchPreset, PatchSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Applied,
    Skipped,
    Failed,
}

use Status::{Applied as A, Failed as F, Skipped as S};

impl From<&PatchStatus> for Status {
    fn from(status: &PatchStatus) -> Self {
        match status {
            PatchStatus::Applied => Self::Applied,
            PatchStatus::Skipped { .. } => Self::Skipped,
            PatchStatus::Failed { .. } => Self::Failed,
        }
    }
}

fn declared(bundle: &str, id: &str, dependencies: &[&str]) -> PatchSpec {
    PatchSpec {
        bundle: bundle.into(),
        id: id.into(),
        name: id.into(),
        hidden: false,
        description: String::new(),
        enabled_by_default: true,
        dependencies: dependencies.iter().map(|d| (*d).into()).collect(),
        compatibility: [CompatiblePackage {
            package: "com.example".into(),
            versions: Vec::new(),
        }]
        .into_iter()
        .collect(),
        options: Vec::new(),
    }
}

fn noops(declarations: impl IntoIterator<Item = PatchSpec>) -> Vec<Patch> {
    declarations
        .into_iter()
        .map(|declaration| Patch::new(declaration, false, |_, _| Ok(())))
        .collect()
}

fn statuses(
    declarations: &[&PatchSpec],
    selection: &PatchSelection,
    package: &str,
    version: &str,
) -> Vec<Status> {
    let patches = noops(
        declarations
            .iter()
            .map(|declaration| (*declaration).clone()),
    );
    validate_patches(
        &patches.iter().collect::<Vec<_>>(),
        selection,
        Some(package),
        Some(version),
    )
    .unwrap()
    .iter()
    .map(|r| Status::from(&r.status))
    .collect()
}

#[test]
fn selection_presets_and_compatibility() {
    let own = declared("bundle", "own", &[]);
    let mut pinned = declared("bundle", "pinned", &[]);
    pinned.compatibility = [CompatiblePackage {
        package: "com.example".into(),
        versions: vec!["1.0".into()],
    }]
    .into_iter()
    .collect();
    let mut optional = declared("bundle", "optional", &[]);
    optional.enabled_by_default = false;
    let mut universal = declared("bundle", "universal", &[]);
    universal.compatibility = Compatibility::Universal;
    let patches = [&own, &pinned, &optional, &universal];
    for (selection, (package, expected)) in [
        (PatchSelection::default(), ("com.example", [A, S, S, S])),
        (
            PatchSelection {
                ignore_versions: true,
                ..Default::default()
            },
            ("com.example", [A, A, S, S]),
        ),
        (
            PatchSelection {
                enable: vec!["universal".into()],
                ..Default::default()
            },
            ("com.example", [A, S, S, A]),
        ),
        (
            PatchSelection {
                preset: PatchPreset::All,
                disable: vec!["own".into()],
                ..Default::default()
            },
            ("com.example", [S, S, A, S]),
        ),
        (
            PatchSelection {
                preset: PatchPreset::None,
                ..Default::default()
            },
            ("com.example", [S; 4]),
        ),
        (PatchSelection::default(), ("com.other", [S; 4])),
    ] {
        assert_eq!(statuses(&patches, &selection, package, "2.0"), expected);
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
    let root = declared("bundle", "root", &["bundle/middle"]);
    for (selection, (package, expected)) in [
        (PatchSelection::default(), ("com.example", [A, A, A])),
        (PatchSelection::default(), ("com.other", [S; 3])),
        (
            PatchSelection {
                disable: vec!["root".into()],
                ..Default::default()
            },
            ("com.example", [S; 3]),
        ),
        (
            PatchSelection {
                disable: vec!["middle".into()],
                ..Default::default()
            },
            ("com.example", [S; 3]),
        ),
    ] {
        assert_eq!(
            statuses(&[&helper, &middle, &root], &selection, package, "2.0"),
            expected
        );
    }
    middle.compatibility = [CompatiblePackage {
        package: "com.other".into(),
        versions: Vec::new(),
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
        [S; 3]
    );
    middle.dependencies.push("missing/helper".into());
    assert_eq!(
        statuses(
            &[&helper, &middle, &root],
            &PatchSelection::default(),
            "com.example",
            "2.0"
        ),
        [S; 3]
    );
}

#[test]
fn reference_selection_disambiguates_bundles() {
    let official = declared("official", "helper", &[]);
    let fork = declared("fork", "helper", &[]);
    let consumer = declared("fork", "consumer", &["official/helper"]);
    let patches = noops([official, fork, consumer]);
    let patches: Vec<_> = patches.iter().collect();
    let selection = PatchSelection {
        preset: PatchPreset::None,
        enable: vec!["consumer".into()],
        ..Default::default()
    };
    let results = validate_patches(&patches, &selection, Some("com.example"), None).unwrap();
    assert_eq!(
        results
            .iter()
            .filter(|r| r.status == PatchStatus::Applied)
            .map(|r| r.patch.as_str())
            .collect::<Vec<_>>(),
        ["official/helper", "fork/consumer"]
    );
    let selection = PatchSelection {
        enable: vec!["helper".into()],
        ..selection
    };
    assert!(matches!(
        validate_patches(&patches, &selection, Some("com.example"), None),
        Err(PatcherError::InvalidSelection(_))
    ));
}

#[test]
fn explicitly_selected_missing_dependencies_are_errors() {
    let consumer = declared("fork", "consumer", &["official/helper"]);
    let other = declared("official", "other", &[]);
    for patches in [
        vec![consumer.clone()],
        vec![consumer.clone(), other.clone()],
    ] {
        let patches = noops(patches);
        let patches: Vec<_> = patches.iter().collect();
        let results = validate_patches(
            &patches,
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
            validate_patches(&patches, &selection, Some("com.example"), None),
            Err(PatcherError::MissingBundle { .. } | PatcherError::MissingDependency { .. })
        ));
    }
}

#[derive(Clone, Copy)]
enum Failure {
    None,
    Execute,
    Finalize,
}

fn hooked(declaration: PatchSpec, failure: Failure, events: Arc<Mutex<Vec<String>>>) -> Patch {
    let id = declaration.id.clone();
    Patch::new(declaration, true, move |phase, _| {
        let name = match phase {
            PatchPhase::Execute => "execute",
            PatchPhase::Finalize => "finalize",
        };
        events.lock().unwrap().push(format!("{name}:{id}"));
        if matches!(
            (phase, failure),
            (PatchPhase::Execute, Failure::Execute) | (PatchPhase::Finalize, Failure::Finalize)
        ) {
            return Err(PatcherError::Bridge("fixture callback failed".into()));
        }
        Ok(())
    })
}

#[test]
fn finalization_orders_hooks_and_emits_one_terminal_result() {
    for (failure, expected) in [
        (Failure::None, [A, A, A]),
        (Failure::Execute, [F, A, S]),
        (Failure::Finalize, [F, A, A]),
    ] {
        let (_dir, mut apk) = crate::test_support::apk();
        let events = Arc::new(Mutex::new(Vec::new()));
        let dependency = hooked(
            declared("bundle", "dependency", &[]),
            failure,
            events.clone(),
        );
        let leaf = hooked(
            declared("bundle", "leaf", &[]),
            Failure::None,
            events.clone(),
        );
        let consumer = hooked(
            declared("bundle", "consumer", &["bundle/dependency"]),
            Failure::None,
            events.clone(),
        );
        let mut finished = Vec::new();
        let results = apply_patches(
            &mut PatchContext::new(&mut apk),
            &[&consumer, &dependency, &leaf],
            &PatchSelection::default(),
            |event| {
                if let ProgressEvent::Finished { patch, status } = event {
                    events.lock().unwrap().push(format!("finished:{patch}"));
                    finished.push((patch, Status::from(&status)));
                }
            },
        )
        .unwrap();
        assert_eq!(
            results
                .iter()
                .map(|r| Status::from(&r.status))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(finished.len(), results.len());
        for result in &results {
            assert_eq!(
                finished
                    .iter()
                    .filter(|(patch, _)| *patch == result.patch)
                    .map(|(_, status)| *status)
                    .collect::<Vec<_>>(),
                [Status::from(&result.status)]
            );
        }
        let events = events.lock().unwrap();
        let position = |event: &str| events.iter().position(|value| value == event).unwrap();
        assert!(position("finalize:leaf") < position("finished:bundle/leaf"));
        if matches!(failure, Failure::Execute) {
            assert!(
                !events
                    .iter()
                    .any(|event| event == "finalize:dependency" || event == "finalize:consumer")
            );
        } else {
            assert!(position("finalize:consumer") < position("finalize:dependency"));
            assert!(position("finalize:dependency") < position("finished:bundle/dependency"));
        }
    }
}
