// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod spec;

use std::path::PathBuf;

use crate::context::PatchContext;
use crate::error::Result;
use crate::options::OptionDeclaration;

pub use spec::CompatibilitySpec as Compatibility;
pub use spec::{CompatibilitySpec, PatchId, PatchSpec};

pub trait Patch: Send + Sync {
    fn spec(&self) -> &PatchSpec;

    fn id(&self) -> &PatchId {
        &self.spec().id
    }

    fn name(&self) -> &str {
        self.id().as_str()
    }

    fn description(&self) -> &str {
        self.spec().description.as_ref()
    }

    fn compatible_with(&self) -> &[CompatibilitySpec] {
        &self.spec().compatibility
    }

    fn enabled_by_default(&self) -> bool {
        self.spec().enabled_by_default
    }

    fn depends_on(&self) -> &[PatchId] {
        &self.spec().dependencies
    }

    fn options(&self) -> &[OptionDeclaration] {
        &self.spec().options
    }

    fn extension_dex(&self) -> &[PathBuf] {
        &self.spec().extension_dex
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;

    fn after_dependents(&self, _ctx: &mut PatchContext) -> Result<()> {
        Ok(())
    }
}
