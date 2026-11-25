// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::InstallMethod;

use crate::context::PatchContext;

/// How a run's output reaches the device, which decides what a patch may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The output installs as an app of its own.
    Install,
    /// The output replaces the installed app's files, and the system keeps the
    /// manifest it read from the installed app. A patch that edits the manifest, or
    /// native code of an app that loads the native libraries it extracted at install,
    /// finishes unmountable: the run undoes it and leaves out the patches that need it.
    /// A patch that rewrites the output is unmountable before it runs.
    Mount,
}

impl From<InstallMethod> for Delivery {
    fn from(method: InstallMethod) -> Self {
        match method {
            InstallMethod::Install => Self::Install,
            InstallMethod::Mount => Self::Mount,
        }
    }
}

/// The revisions of what a mount cannot deliver, taken before a patch runs.
#[derive(Clone, Copy)]
pub(super) struct Revisions {
    manifest: u64,
    native: usize,
}

impl Revisions {
    pub(super) fn of(ctx: &PatchContext<'_>) -> Self {
        Self {
            manifest: ctx.apk().manifest_revision(),
            native: ctx.native_revision(),
        }
    }
}

impl Delivery {
    /// Why a patch that ran from `before` cannot be delivered, when it changed what this
    /// delivery does not apply.
    pub(super) fn unmountable(
        self,
        before: Revisions,
        ctx: &PatchContext<'_>,
    ) -> Option<&'static str> {
        if self != Self::Mount {
            return None;
        }
        if ctx.apk().manifest_revision() != before.manifest {
            return Some(CHANGES_MANIFEST);
        }
        (ctx.native_revision() != before.native
            && ctx.apk().base().manifest().extract_native_libs())
        .then_some(EDITS_EXTRACTED_NATIVE_CODE)
    }
}

const CHANGES_MANIFEST: &str = "changes the manifest, which a mount does not apply";
const EDITS_EXTRACTED_NATIVE_CODE: &str = "edits native code the installed app loads from \
     libraries it extracted at install, which a mount does not replace";
pub(super) const REWRITES_OUTPUT: &str = "rewrites the written APK, which a mount cannot check";
