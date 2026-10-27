// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{Arc, Mutex};

use super::*;

/// A patch that adds `assets/<id>`, counts its runs in `runs`, and then does what `extra` does.
fn adding(
    declaration: PatchSpec,
    runs: Arc<Mutex<Vec<String>>>,
    extra: impl Fn(&mut PatchContext<'_>) -> Result<()> + Send + Sync + 'static,
) -> Patch {
    let id = declaration.reference.id.clone();
    Patch::new(declaration, false, move |phase| {
        let PatchPhase::Execute(ctx) = phase else {
            unreachable!("fixture patches only execute");
        };
        runs.lock().unwrap().push(id.clone());
        ctx.inject_file(
            0,
            &format!("assets/{id}"),
            id.clone().into_bytes(),
            reseam_apk::Compression::Deflated,
        )?;
        extra(ctx)
    })
}

fn edit_manifest(ctx: &mut PatchContext<'_>) {
    ctx.apk_mut()
        .base_mut()
        .manifest_mut()
        .set_version_name("edited");
}

struct Outcome {
    results: Vec<(String, Status)>,
    runs: Vec<String>,
    reverted: Vec<String>,
    assets: Vec<String>,
    version: Option<String>,
}

fn run_patches(delivery: Delivery, patches: &[&Patch], runs: &Arc<Mutex<Vec<String>>>) -> Outcome {
    let (_dir, mut apk) = crate::test_support::apk();
    let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
    let plan = ResolvedPlan::resolve(
        &specs,
        &PatchSelection::default(),
        delivery,
        Some("com.example"),
        ApkVersion {
            name: Some("2.0"),
            code: None,
        },
    )
    .unwrap();
    let mut reverted = Vec::new();
    let results = apply_patches(
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
        &specs,
        plan,
        patches,
        |event| {
            if let RunEvent::PatchesReverted { patches } = event {
                reverted.extend(patches.iter().map(ToString::to_string));
            }
        },
    )
    .unwrap()
    .into_results();
    Outcome {
        results: results
            .iter()
            .map(|result| (result.patch.to_string(), Status::from(&result.status)))
            .collect(),
        runs: runs.lock().unwrap().clone(),
        reverted,
        assets: apk
            .entry_names()
            .into_iter()
            .filter(|name| name.starts_with("assets/"))
            .collect(),
        version: apk.version_name().map(std::borrow::Cow::into_owned),
    }
}

#[test]
fn a_patch_that_cannot_apply_leaves_nothing_behind() {
    use Status::Unmountable as U;
    for (delivery, expected) in [(Delivery::Install, F), (Delivery::Mount, U)] {
        let runs = Arc::new(Mutex::new(Vec::new()));
        let failing = adding(
            declared("bundle", "failing", &[]),
            runs.clone(),
            move |ctx| {
                edit_manifest(ctx);
                match delivery {
                    Delivery::Install => Err(PatcherError::Bridge("fixture failed".into())),
                    Delivery::Mount => Ok(()),
                }
            },
        );
        let independent = adding(declared("bundle", "independent", &[]), runs.clone(), |_| {
            Ok(())
        });
        let dependent = adding(
            declared("bundle", "dependent", &["bundle/failing"]),
            runs.clone(),
            |_| Ok(()),
        );
        let outcome = run_patches(delivery, &[&failing, &independent, &dependent], &runs);
        assert_eq!(
            outcome.results,
            covered(&[("failing", expected), ("independent", A), ("dependent", S)])
        );
        assert_eq!(outcome.runs, ["failing", "independent"]);
        assert!(outcome.reverted.is_empty());
        assert_eq!(outcome.assets, ["assets/independent"]);
        assert_eq!(outcome.version.as_deref(), Some("2.0"));
    }
}

#[test]
fn a_dependency_only_a_left_out_patch_needed_is_undone() {
    let runs = Arc::new(Mutex::new(Vec::new()));
    let mut helper = declared("bundle", "helper", &[]);
    helper.enabled_by_default = false;
    let helper = adding(helper, runs.clone(), |_| Ok(()));
    let consumer = adding(
        declared("bundle", "consumer", &["bundle/helper"]),
        runs.clone(),
        |ctx| {
            edit_manifest(ctx);
            Ok(())
        },
    );
    let other = adding(declared("bundle", "other", &[]), runs.clone(), |_| Ok(()));
    let outcome = run_patches(Delivery::Mount, &[&helper, &consumer, &other], &runs);
    assert_eq!(
        outcome.results,
        covered(&[("other", A), ("consumer", Status::Unmountable)])
    );
    assert_eq!(outcome.runs, ["helper", "other", "consumer", "other"]);
    assert_eq!(outcome.reverted, ["bundle/helper", "bundle/other"]);
    assert_eq!(outcome.assets, ["assets/other"]);
    assert_eq!(outcome.version.as_deref(), Some("2.0"));
}
