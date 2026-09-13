// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::context::PatchContext;
use crate::error::Result;

pub trait Patch: Send + Sync {
    fn spec(&self) -> &PatchSpec;

    /// `<bundle>/<id>`: the identity dependencies, selections, and results refer to.
    fn reference(&self) -> String {
        self.spec().reference()
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;

    /// Runs after every patch depending on this one has executed.
    fn after_dependents(&self, _ctx: &mut PatchContext) -> Result<()> {
        Ok(())
    }
}

pub use reseam_model::{is_slug, Compatibility, CompatiblePackage, PatchSpec};
