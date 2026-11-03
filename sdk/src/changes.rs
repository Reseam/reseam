// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use reseam_model::{ChangesRequest, PatchChange, PatchSpec, PlannedRun};

use crate::error::Result;
use crate::plan::plan_specs;

/// How patching the app of `request.record` again, with the same selection
/// and the patches available now, would differ from that build: patches it
/// took up that changed or are no longer taken up, in the build's order, then
/// patches it lacks. Empty when it would make the same build.
pub fn changes(request: &ChangesRequest) -> Result<Vec<PatchChange>> {
    let record = &request.record;
    let specs: Vec<&PatchSpec> = request.patches.iter().map(|patch| &patch.spec).collect();
    let planned = plan_specs(
        &specs,
        &record.app,
        &record.selection,
        record.install_method,
    )?;
    let fingerprints: HashMap<_, _> = request
        .patches
        .iter()
        .map(|patch| (patch.spec.reference(), patch.fingerprint.as_str()))
        .collect();
    let mut taken: HashMap<_, _> = planned
        .patches
        .iter()
        .filter(|patch| patch.run != PlannedRun::Off)
        .map(|patch| (&patch.patch, fingerprints[&patch.patch]))
        .collect();
    let mut changes = Vec::new();
    for recorded in &record.patches {
        match taken.remove(&recorded.patch) {
            None => changes.push(PatchChange::Removed {
                patch: recorded.patch.clone(),
            }),
            Some(fingerprint) if fingerprint != recorded.fingerprint => {
                changes.push(PatchChange::Changed {
                    patch: recorded.patch.clone(),
                });
            }
            Some(_) => {}
        }
    }
    changes.extend(
        planned
            .patches
            .iter()
            .filter(|patch| taken.contains_key(&patch.patch))
            .map(|patch| PatchChange::Added {
                patch: patch.patch.clone(),
            }),
    );
    Ok(changes)
}
