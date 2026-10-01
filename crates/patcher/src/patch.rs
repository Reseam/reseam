// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::context::PatchContext;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchPhase {
    /// Runs after every dependency executed successfully.
    Execute,
    /// Runs once after successful execution, after the patch's dependents finalized.
    Finalize,
}

type Callback = dyn Fn(PatchPhase, &mut PatchContext<'_>) -> Result<()> + Send + Sync;

/// Execution and finalization errors fail this patch; edits already made remain
/// in the context. The engine catches callback panics and skips dependents of an
/// execution failure. A callback can implement either phase as a no-op.
pub struct Patch {
    spec: PatchSpec,
    reference: String,
    callback: Box<Callback>,
}

impl Patch {
    /// Creates a patch, retaining the callback and everything it captures until
    /// the patch is dropped. Dependency and option validation happens when the
    /// engine resolves a selection.
    pub fn new(
        spec: PatchSpec,
        callback: impl Fn(PatchPhase, &mut PatchContext<'_>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            reference: spec.reference(),
            spec,
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

    /// Invokes a phase directly. Callers needing dependency ordering, option
    /// validation and panic isolation should use `engine::apply_patches`.
    pub fn invoke(&self, phase: PatchPhase, context: &mut PatchContext<'_>) -> Result<()> {
        (self.callback)(phase, context)
    }
}

pub use reseam_model::{Compatibility, CompatiblePackage, PatchPreset, PatchSpec, is_slug};
