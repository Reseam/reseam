// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::OptionDeclaration;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[boltffi::data]
pub struct CompatiblePackage {
    pub package: String,
    /// Empty means every version.
    pub versions: Vec<String>,
}

/// Which apps a patch declares itself for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum Compatibility {
    /// Declares no package, so it applies to every app and stays opt-in.
    Universal,
    Packages {
        packages: Vec<CompatiblePackage>,
    },
}

impl Compatibility {
    pub fn is_universal(&self) -> bool {
        matches!(self, Self::Universal)
    }
}

impl FromIterator<CompatiblePackage> for Compatibility {
    fn from_iter<I: IntoIterator<Item = CompatiblePackage>>(packages: I) -> Self {
        let packages: Vec<_> = packages.into_iter().collect();
        if packages.is_empty() {
            Self::Universal
        } else {
            Self::Packages { packages }
        }
    }
}

/// A bulk selection clients offer next to toggling patches one by one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchPreset {
    /// The patches their authors enable by default.
    #[default]
    Recommended,
    All,
    None,
}

impl std::str::FromStr for PatchPreset {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "recommended" => Ok(Self::Recommended),
            "all" => Ok(Self::All),
            "none" => Ok(Self::None),
            _ => Err(format!(
                "unknown preset '{value}'; expected recommended, all or none"
            )),
        }
    }
}

/// Bundle names and patch IDs: lowercase letters and digits, single hyphens between them.
pub fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchSpec {
    pub bundle: String,
    /// `<package>.<property>` of the public declaration; unique within the bundle.
    pub id: String,
    /// What users see. Different patches may share a display name.
    pub name: String,
    /// Hidden patches are dependencies of other patches: never listed to
    /// users and never selected on their own.
    pub hidden: bool,
    pub description: String,
    pub enabled_by_default: bool,
    /// References (`<bundle>/<id>`) to the patches that run first.
    pub dependencies: Vec<String>,
    pub compatibility: Compatibility,
    pub options: Vec<OptionDeclaration>,
}

impl PatchSpec {
    pub fn reference(&self) -> String {
        format!("{}/{}", self.bundle, self.id)
    }

    /// Why the patch does not apply to `package`/`version`, if it does not.
    pub fn incompatibility(&self, package: Option<&str>, version: Option<&str>) -> Option<String> {
        let entries = match self.package_compatibility(package) {
            Ok(Some(entries)) => entries,
            Ok(None) => return None,
            Err(reason) => return Some(reason),
        };
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

    /// Whether `preset` selects the patch for an app of `package`. Presets only
    /// take patches that declare the package, so universal patches stay opt-in.
    /// Version compatibility is not considered; it is reported separately.
    pub fn in_preset(&self, preset: PatchPreset, package: Option<&str>) -> bool {
        let declared = matches!(self.package_compatibility(package), Ok(Some(_)));
        declared
            && !self.hidden
            && match preset {
                PatchPreset::Recommended => self.enabled_by_default,
                PatchPreset::All => true,
                PatchPreset::None => false,
            }
    }

    /// Why the patch does not apply to `package` at any version, if it does not.
    pub fn package_incompatibility(&self, package: Option<&str>) -> Option<String> {
        self.package_compatibility(package).err()
    }

    /// The declared entries for `package`; `Ok(None)` when the patch applies
    /// to every app.
    fn package_compatibility(
        &self,
        package: Option<&str>,
    ) -> std::result::Result<Option<Vec<&CompatiblePackage>>, String> {
        let Compatibility::Packages { packages } = &self.compatibility else {
            return Ok(None);
        };
        let Some(package) = package else {
            return Err("APK has no package name".to_owned());
        };
        let entries: Vec<&CompatiblePackage> = packages
            .iter()
            .filter(|entry| entry.package == package)
            .collect();
        if entries.is_empty() {
            return Err(format!("incompatible package: {package}"));
        }
        Ok(Some(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(compatibility: Compatibility, enabled_by_default: bool, hidden: bool) -> PatchSpec {
        PatchSpec {
            bundle: "bundle".to_owned(),
            id: "patch".to_owned(),
            name: "Patch".to_owned(),
            hidden,
            description: String::new(),
            enabled_by_default,
            dependencies: Vec::new(),
            compatibility,
            options: Vec::new(),
        }
    }

    fn presets(spec: &PatchSpec, package: Option<&str>) -> Vec<PatchPreset> {
        [
            PatchPreset::Recommended,
            PatchPreset::All,
            PatchPreset::None,
        ]
        .into_iter()
        .filter(|&preset| spec.in_preset(preset, package))
        .collect()
    }

    #[test]
    fn presets_take_only_visible_patches_that_declare_the_package() {
        let declared = |versions: &[&str]| {
            [CompatiblePackage {
                package: "com.example".to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            }]
            .into_iter()
            .collect()
        };
        let app = Some("com.example");
        use PatchPreset::{All, Recommended};

        assert_eq!(
            presets(&spec(declared(&[]), true, false), app),
            vec![Recommended, All]
        );
        assert_eq!(
            presets(&spec(declared(&["1.0"]), true, false), app),
            vec![Recommended, All]
        );
        assert_eq!(presets(&spec(declared(&[]), false, false), app), vec![All]);
        assert_eq!(
            presets(&spec(declared(&[]), true, false), Some("com.other")),
            vec![]
        );
        assert_eq!(presets(&spec(declared(&[]), true, false), None), vec![]);
        assert_eq!(presets(&spec(declared(&[]), false, true), app), vec![]);
        assert_eq!(
            presets(&spec(Compatibility::Universal, true, false), app),
            vec![]
        );
    }
}
