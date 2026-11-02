// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::analysis::{Inspected, KotlinCompiler, Snippet};
use reseam_patcher::context::{ExtensionSet, PatchContext};

use super::Analysis;
use crate::error::Result;

impl Analysis {
    /// Runs a target expression or a snippet against the APK in a patch
    /// runtime, compiled with `compiler`. A target matches what it matches in
    /// a patch; anything a snippet changes is undone before this returns.
    pub fn inspect(&mut self, compiler: &KotlinCompiler, snippet: &Snippet) -> Result<Inspected> {
        let mut ctx = PatchContext::new(&mut self.opened.apk, ExtensionSet::load(&[])?);
        Ok(reseam_patcher::inspect::inspect(
            &mut ctx, compiler, snippet,
        )?)
    }
}
