// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::Serialize;

use crate::context::PatchContext;
use crate::error::Result;
use crate::options::OptionDeclaration;

pub trait Patch: Send + Sync {
    fn spec(&self) -> &PatchSpec;

    fn name(&self) -> &str {
        &self.spec().id
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;

    /// Runs after every patch depending on this one has executed.
    fn after_dependents(&self, _ctx: &mut PatchContext) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Compatibility {
    pub package: String,
    /// Empty means every version.
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchSpec {
    pub id: String,
    pub description: String,
    pub enabled_by_default: bool,
    pub dependencies: Vec<String>,
    /// Empty means every app.
    pub compatibility: Vec<Compatibility>,
    pub options: Vec<OptionDeclaration>,
    #[serde(skip)]
    pub extension_dex: Vec<PathBuf>,
}

impl PatchSpec {
    /// Why the patch does not apply to `package`/`version`, if it does not.
    pub fn incompatibility(&self, package: Option<&str>, version: Option<&str>) -> Option<String> {
        if self.compatibility.is_empty() {
            return None;
        }
        let Some(package) = package else {
            return Some("APK has no package name".to_owned());
        };
        let Some(entry) = self
            .compatibility
            .iter()
            .find(|entry| entry.package == package)
        else {
            return Some(format!("incompatible package: {package}"));
        };
        if entry.versions.is_empty() {
            return None;
        }
        match version {
            Some(version) if entry.versions.iter().any(|allowed| allowed == version) => None,
            Some(version) => Some(format!(
                "expected one of [{}], got {version}",
                entry.versions.join(", ")
            )),
            None => Some("APK has no version name".to_owned()),
        }
    }
}
