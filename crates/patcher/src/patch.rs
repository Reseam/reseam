// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;

use crate::context::PatchContext;
use crate::error::Result;
use crate::options::OptionDeclaration;

pub trait Patch: Send + Sync {
    fn spec(&self) -> &PatchSpec;

    /// The identity dependencies and selections refer to.
    fn id(&self) -> &str {
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
    /// What users see. Equals `id` unless the patch is hidden.
    pub name: String,
    /// Hidden patches are dependencies of other patches: never listed to
    /// users and never selected on their own.
    pub hidden: bool,
    pub description: String,
    pub enabled_by_default: bool,
    pub dependencies: Vec<String>,
    /// Empty means every app.
    pub compatibility: Vec<Compatibility>,
    pub options: Vec<OptionDeclaration>,
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
        let entries: Vec<&Compatibility> = self
            .compatibility
            .iter()
            .filter(|entry| entry.package == package)
            .collect();
        if entries.is_empty() {
            return Some(format!("incompatible package: {package}"));
        }
        if entries.iter().any(|entry| entry.versions.is_empty()) {
            return None;
        }
        let allowed: Vec<&str> = entries
            .iter()
            .flat_map(|entry| entry.versions.iter().map(String::as_str))
            .collect();
        match version {
            Some(version) if allowed.contains(&version) => None,
            Some(version) => Some(format!(
                "expected one of [{}], got {version}",
                allowed.join(", ")
            )),
            None => Some("APK has no version name".to_owned()),
        }
    }
}
