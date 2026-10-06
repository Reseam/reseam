// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::context::PatchContext;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchPhase {
    /// Runs after every dependency executed successfully.
    Execute,
    /// Runs once after successful execution, after the patch's dependents finalized, for patches
    /// that finalize.
    Finalize,
}

type Callback = dyn Fn(PatchPhase, &mut PatchContext<'_>) -> Result<()> + Send + Sync;

/// Execution and finalization errors fail this patch; edits already made remain
/// in the context. The engine catches callback panics and skips dependents of an
/// execution failure.
pub struct Patch {
    spec: PatchSpec,
    reference: String,
    finalizes: bool,
    callback: Box<Callback>,
}

impl Patch {
    /// Creates a patch, retaining the callback and everything it captures until
    /// the patch is dropped. Dependency and option validation happens when the
    /// engine resolves a selection. Without `finalizes`, the callback never sees
    /// [`PatchPhase::Finalize`] and the patch's result is final once it executes.
    pub fn new(
        spec: PatchSpec,
        finalizes: bool,
        callback: impl Fn(PatchPhase, &mut PatchContext<'_>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            reference: spec.reference(),
            spec,
            finalizes,
            callback: Box::new(callback),
        }
    }

    pub fn spec(&self) -> &PatchSpec {
        &self.spec
    }

    /// `<bundle>/<id>`, shared by selections, dependencies and results.
    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn finalizes(&self) -> bool {
        self.finalizes
    }

    /// Invokes a phase directly. Callers needing dependency ordering, option
    /// validation and panic isolation should use `engine::apply_patches`.
    pub fn invoke(&self, phase: PatchPhase, context: &mut PatchContext<'_>) -> Result<()> {
        (self.callback)(phase, context)
    }
}

pub use reseam_model::{Compatibility, CompatiblePackage, PatchPreset, PatchSpec, is_slug};
