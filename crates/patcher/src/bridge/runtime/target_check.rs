// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A check of a bundle's targets: how each one resolves in the patch runs on
//! this thread, and which no patch tried to resolve. Runs outside a check
//! record nothing.

use jni::{jni_sig, jni_str};
use reseam_model::analysis::TargetResolution;

use super::inspector::call_inspector;
use super::jni_values::string_list;
use super::jvm;
use crate::bridge::inspection::{start_recording, stop_recording};
use crate::error::Result;

/// A running check, from before the checked bundles load until [`Self::finish`].
pub struct TargetCheck(());

/// What a check found.
pub struct CheckedTargets {
    pub resolutions: Vec<TargetResolution>,
    /// The labels of targets created during the check that no patch tried to resolve.
    pub unreached: Vec<String>,
}

impl TargetCheck {
    pub fn start() -> Result<Self> {
        call_inspector(|env, _, inspector| {
            let call = env.call_static_method(
                inspector,
                jni_str!("startTargetCheck"),
                jni_sig!("()V"),
                &[],
            );
            jvm::outcome(env, call, "Inspector.startTargetCheck").map(drop)
        })?;
        start_recording();
        Ok(Self(()))
    }

    pub fn finish(self) -> Result<CheckedTargets> {
        let resolutions = stop_recording();
        drop(self);
        let unreached = call_inspector(|env, _, inspector| {
            let call = env.call_static_method(
                inspector,
                jni_str!("finishTargetCheck"),
                jni_sig!("()[Ljava/lang/String;"),
                &[],
            );
            let array = jvm::outcome(env, call, "Inspector.finishTargetCheck")?.l()?;
            string_list(env, array)
        })?;
        Ok(CheckedTargets {
            resolutions,
            unreached,
        })
    }
}

impl Drop for TargetCheck {
    fn drop(&mut self) {
        stop_recording();
    }
}
