// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{OptionDeclaration, Problem};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[boltffi::data]
pub struct CompatiblePackage {
    pub package: String,
    /// Empty means every version.
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum Compatibility {
    /// Declares no package, so it applies to every app and joins presets only when enabled by default.
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
    type Err = Problem;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "recommended" => Ok(Self::Recommended),
            "all" => Ok(Self::All),
            "none" => Ok(Self::None),
            _ => Err(Problem::UnknownPreset {
                value: value.to_owned(),
            }),
        }
    }
}

/// A nonempty slug: lowercase letters or digits separated by single hyphens.
pub fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    pub fn incompatibility(&self, package: Option<&str>, version: Option<&str>) -> Option<String> {
        let entries = match self.package_compatibility(package) {
            Ok(Some(entries)) => entries,
            Ok(None) => return None,
            Err(reason) => return Some(reason.to_string()),
        };
        if entries
            .iter()
            .filter(|entry| Some(entry.package.as_str()) == package)
            .any(|entry| entry.versions.is_empty())
        {
            return None;
        }
        let allowed: Vec<&str> = entries
            .iter()
            .filter(|entry| Some(entry.package.as_str()) == package)
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

    /// Whether `preset` selects the patch for an app of `package`. Presets take
    /// the patches that declare the package, and a universal patch only when its
    /// author enables it by default. Version compatibility is not considered; it
    /// is reported separately.
    pub fn in_preset(&self, preset: PatchPreset, package: Option<&str>) -> bool {
        let declared = matches!(self.package_compatibility(package), Ok(Some(_)));
        let universal = matches!(self.compatibility, Compatibility::Universal);
        !self.hidden
            && match preset {
                PatchPreset::Recommended => (declared || universal) && self.enabled_by_default,
                PatchPreset::All => declared || universal && self.enabled_by_default,
                PatchPreset::None => false,
            }
    }

    pub fn package_incompatibility(&self, package: Option<&str>) -> Option<String> {
        self.package_compatibility(package)
            .err()
            .map(|error| error.to_string())
    }

    fn package_compatibility(
        &self,
        package: Option<&str>,
    ) -> crate::Result<Option<&[CompatiblePackage]>> {
        let Compatibility::Packages { packages } = &self.compatibility else {
            return Ok(None);
        };
        let Some(package) = package else {
            return Err(Problem::MissingPackage);
        };
        if !packages.iter().any(|entry| entry.package == package) {
            return Err(Problem::IncompatiblePackage {
                package: package.to_owned(),
            });
        }
        Ok(Some(packages))
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
    fn presets_take_visible_patches_that_declare_the_package_or_opt_in() {
        use PatchPreset::{All, Recommended};
        let declared = |versions: &[&str]| {
            [CompatiblePackage {
                package: "com.example".to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            }]
            .into_iter()
            .collect()
        };
        let app = Some("com.example");

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
            vec![Recommended, All]
        );
        assert_eq!(
            presets(&spec(Compatibility::Universal, false, false), app),
            vec![]
        );
        assert_eq!(
            presets(&spec(Compatibility::Universal, true, true), app),
            vec![]
        );
    }
}
