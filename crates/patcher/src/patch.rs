// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::context::PatchContext;
use crate::error::Result;
use crate::output::OutputContext;

/// A phase of a patch and what it works on.
pub enum PatchPhase<'a, 'b> {
    /// Runs after every dependency executed successfully.
    Execute(&'a mut PatchContext<'b>),
    /// Runs once after successful execution, after the patch's dependents finalized, for patches
    /// that finalize.
    Finalize(&'a mut PatchContext<'b>),
    /// Runs after the host wrote the APK and before it signs it, for patches whose spec
    /// [rewrites output](PatchSpec::rewrites_output).
    RewriteOutput(&'a mut OutputContext<'b>),
}

type Callback = dyn Fn(PatchPhase<'_, '_>) -> Result<()> + Send + Sync;

/// Execution and finalization errors fail this patch; edits already made remain
/// in the context. The engine catches callback panics and skips dependents of an
/// execution failure.
pub struct Patch {
    spec: PatchSpec,
    finalizes: bool,
    callback: Box<Callback>,
}

impl Patch {
    /// Creates a patch, retaining the callback and everything it captures until
    /// the patch is dropped. Dependency and option validation happens when the
    /// engine resolves a selection. Without `finalizes`, the callback never sees
    /// [`PatchPhase::Finalize`] and the patch's result is final once it executes.
    /// It sees [`PatchPhase::RewriteOutput`] only when the spec rewrites output.
    pub fn new(
        spec: PatchSpec,
        finalizes: bool,
        callback: impl Fn(PatchPhase<'_, '_>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            spec,
            finalizes,
            callback: Box::new(callback),
        }
    }

    pub fn spec(&self) -> &PatchSpec {
        &self.spec
    }

    pub fn reference(&self) -> &PatchRef {
        &self.spec.reference
    }

    pub fn finalizes(&self) -> bool {
        self.finalizes
    }

    /// Invokes a phase directly. Callers needing dependency ordering, option
    /// validation and panic isolation should use `engine::apply_patches`.
    pub fn invoke(&self, phase: PatchPhase<'_, '_>) -> Result<()> {
        (self.callback)(phase)
    }
}

pub use reseam_model::{
    ApkVersion, Compatibility, CompatiblePackage, PatchPreset, PatchRef, PatchSpec,
};
