// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{Arc, Mutex};

use super::*;

#[derive(Clone, Copy)]
enum Failure {
    None,
    Execute,
    Finalize,
}

fn hooked(declaration: PatchSpec, failure: Failure, events: Arc<Mutex<Vec<String>>>) -> Patch {
    let id = declaration.reference.id.clone();
    Patch::new(declaration, true, move |phase| {
        let (name, fails) = match phase {
            PatchPhase::Execute(_) => ("execute", matches!(failure, Failure::Execute)),
            PatchPhase::Finalize(_) => ("finalize", matches!(failure, Failure::Finalize)),
            PatchPhase::RewriteOutput(_) => unreachable!("fixture specs do not rewrite output"),
        };
        events.lock().unwrap().push(format!("{name}:{id}"));
        if fails {
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
        (Failure::Finalize, [F, A, S]),
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
        let patches = [&consumer, &dependency, &leaf];
        let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
        let plan = ResolvedPlan::resolve(
            &specs,
            &PatchSelection::default(),
            Delivery::Install,
            Some("com.example"),
            ApkVersion {
                name: Some("2.0"),
                code: None,
            },
        )
        .unwrap();
        let results = apply_patches(
            &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
            &specs,
            plan,
            &patches,
            |event| match event {
                RunEvent::PatchFinished { patch, status } => {
                    events.lock().unwrap().push(format!("finished:{patch}"));
                    finished.push((patch, Status::from(&status)));
                }
                RunEvent::PatchesReverted { patches } => {
                    finished.retain(|(patch, _)| !patches.contains(patch));
                }
                _ => {}
            },
        )
        .unwrap()
        .into_results();
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
