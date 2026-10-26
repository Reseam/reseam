// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct CompatiblePackage {
    pub package: String,
    /// Accepted version names. With `version_codes` also empty, every version.
    pub versions: Vec<String>,
    /// Accepted version codes; an APK matches when its name or its code is listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub version_codes: Vec<i32>,
}

impl CompatiblePackage {
    fn every_version(&self) -> bool {
        self.versions.is_empty() && self.version_codes.is_empty()
    }
}

/// The version an APK's manifest declares.
#[derive(Debug, Clone, Copy)]
pub struct ApkVersion<'a> {
    pub name: Option<&'a str>,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum Compatibility {
    /// Declares no package, so it applies to every app and joins presets only when enabled by default.
    Universal,
    Packages {
        packages: Vec<CompatiblePackage>,
    },
}

/// Why a patch does not apply to an APK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum Incompatibility {
    /// The patch is for named packages and the APK declares none.
    MissingPackage,
    /// The patch is not for the APK's package.
    Package { package: String },
    /// The patch is for the APK's package, but for other versions of it.
    Version {
        expected_names: Vec<String>,
        expected_codes: Vec<i32>,
        name: Option<String>,
        code: Option<i32>,
    },
}

impl Compatibility {
    pub fn is_universal(&self) -> bool {
        matches!(self, Self::Universal)
    }

    /// Whether the patch names `package` among the packages it is for.
    pub fn declares(&self, package: Option<&str>) -> bool {
        matches!(self.declared_entries(package), Ok(Some(_)))
    }

    /// Why the patch does not apply to an APK of `package`, regardless of its version.
    pub fn package_incompatibility(&self, package: Option<&str>) -> Option<Incompatibility> {
        self.declared_entries(package).err()
    }

    pub fn incompatibility(
        &self,
        package: Option<&str>,
        version: ApkVersion<'_>,
    ) -> Option<Incompatibility> {
        let entries = match self.declared_entries(package) {
            Ok(Some(entries)) => entries,
            Ok(None) => return None,
            Err(reason) => return Some(reason),
        };
        if entries.iter().any(|entry| entry.every_version()) {
            return None;
        }
        let expected_names: Vec<String> = entries
            .iter()
            .flat_map(|entry| entry.versions.iter().cloned())
            .collect();
        let expected_codes: Vec<i32> = entries
            .iter()
            .flat_map(|entry| entry.version_codes.iter().copied())
            .collect();
        if version
            .name
            .is_some_and(|name| expected_names.iter().any(|expected| expected == name))
            || version
                .code
                .is_some_and(|code| expected_codes.contains(&code))
        {
            return None;
        }
        Some(Incompatibility::Version {
            expected_names,
            expected_codes,
            name: version.name.map(str::to_owned),
            code: version.code,
        })
    }

    /// The entries naming `package`, none for a universal patch.
    fn declared_entries(
        &self,
        package: Option<&str>,
    ) -> Result<Option<Vec<&CompatiblePackage>>, Incompatibility> {
        let Self::Packages { packages } = self else {
            return Ok(None);
        };
        let Some(package) = package else {
            return Err(Incompatibility::MissingPackage);
        };
        let entries: Vec<_> = packages
            .iter()
            .filter(|entry| entry.package == package)
            .collect();
        if entries.is_empty() {
            return Err(Incompatibility::Package {
                package: package.to_owned(),
            });
        }
        Ok(Some(entries))
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

impl fmt::Display for Incompatibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPackage => f.write_str("APK has no package name"),
            Self::Package { package } => write!(f, "incompatible package: {package}"),
            Self::Version {
                expected_names,
                expected_codes,
                name,
                code,
            } => {
                let mut expected = Vec::new();
                if !expected_names.is_empty() {
                    expected.push(format!("version name [{}]", expected_names.join(", ")));
                }
                if !expected_codes.is_empty() {
                    let codes: Vec<String> = expected_codes.iter().map(i32::to_string).collect();
                    expected.push(format!("version code [{}]", codes.join(", ")));
                }
                write!(f, "expected {}, got ", expected.join(" or "))?;
                match (name, code) {
                    (Some(name), Some(code)) => write!(f, "{name} ({code})"),
                    (Some(name), None) => f.write_str(name),
                    (None, Some(code)) => write!(f, "version code {code}"),
                    (None, None) => f.write_str("an APK with no version"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_versions_match_by_name_or_code() {
        let pinned = |versions: &[&str], version_codes: &[i32]| -> Compatibility {
            [CompatiblePackage {
                package: "com.example".to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
                version_codes: version_codes.to_vec(),
            }]
            .into_iter()
            .collect()
        };
        let version = |name, code| ApkVersion { name, code };
        let app = Some("com.example");
        for (compatibility, version, compatible) in [
            (pinned(&[], &[]), version(None, None), true),
            (pinned(&["1.0"], &[]), version(Some("1.0"), Some(5)), true),
            (pinned(&["1.0"], &[]), version(Some("2.0"), Some(5)), false),
            (pinned(&[], &[5]), version(Some("2.0"), Some(5)), true),
            (pinned(&[], &[5]), version(Some("2.0"), Some(6)), false),
            (pinned(&[], &[5]), version(None, None), false),
            (pinned(&["1.0"], &[5]), version(Some("1.0"), Some(6)), true),
            (pinned(&["1.0"], &[5]), version(Some("2.0"), Some(5)), true),
            (pinned(&["1.0"], &[5]), version(Some("2.0"), Some(6)), false),
        ] {
            assert_eq!(
                compatibility.incompatibility(app, version).is_none(),
                compatible,
                "{compatibility:?} against {version:?}",
            );
        }
    }
}
