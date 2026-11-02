// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What Kotlin reports back while the engine inspects an APK: a target
//! expression's matches, a snippet's output, and how each target of a patch
//! run resolved.

use std::cell::{Cell, RefCell};

use boltffi::export;

use super::phase::with_patch;
use reseam_model::analysis::{TargetInspection, TargetResolution};

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static RESOLUTIONS: RefCell<Vec<TargetResolution>> = const { RefCell::new(Vec::new()) };
    static TARGET: RefCell<Option<TargetInspection>> = const { RefCell::new(None) };
    static OUTPUT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Starts recording how every target of the patch runs on this thread resolves.
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(super) fn start_recording() {
    RESOLUTIONS.with(|resolutions| resolutions.borrow_mut().clear());
    RECORDING.with(|recording| recording.set(true));
}

/// Stops recording and hands over what was recorded.
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(super) fn stop_recording() -> Vec<TargetResolution> {
    RECORDING.with(|recording| recording.set(false));
    RESOLUTIONS.with(|resolutions| std::mem::take(&mut *resolutions.borrow_mut()))
}

/// Forgets a report a run left behind. Recording stays on: a check starts
/// before the patch run it records enters.
pub(super) fn reset() {
    TARGET.with(|target| *target.borrow_mut() = None);
    OUTPUT.with(|output| *output.borrow_mut() = None);
}

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) fn take_target() -> Option<TargetInspection> {
    TARGET.with(|target| target.borrow_mut().take())
}

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) fn take_output() -> Option<String> {
    OUTPUT.with(|output| output.borrow_mut().take())
}

#[export]
pub fn report_target(
    matches: Vec<String>,
    winner: Option<String>,
    many: bool,
    considered: u32,
    reasons: Vec<String>,
    near_misses: Vec<String>,
    failure: Option<String>,
) {
    TARGET.with(|target| {
        *target.borrow_mut() = Some(TargetInspection {
            matches,
            winner,
            many,
            considered,
            reasons,
            near_misses,
            failure,
        });
    });
}

#[export]
pub fn report_output(text: String) {
    OUTPUT.with(|output| *output.borrow_mut() = Some(text));
}

/// Whether patch runs on this thread report how their targets resolve, which
/// only a check of a bundle's targets asks for.
#[export]
pub fn recording_resolutions() -> bool {
    RECORDING.with(Cell::get)
}

#[export]
#[expect(
    clippy::too_many_arguments,
    reason = "flat values: a record would add a generated type to the patch API's public types"
)]
pub fn report_resolution(
    target: String,
    matches: Vec<String>,
    winner: Option<String>,
    many: bool,
    considered: u32,
    reasons: Vec<String>,
    near_misses: Vec<String>,
    failure: Option<String>,
) {
    let patch = with_patch(|scope| Some(scope.log.patch().clone()));
    RESOLUTIONS.with(|resolutions| {
        resolutions.borrow_mut().push(TargetResolution {
            patch,
            target,
            inspection: TargetInspection {
                matches,
                winner,
                many,
                considered,
                reasons,
                near_misses,
                failure,
            },
        });
    });
}
