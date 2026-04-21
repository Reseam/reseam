// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Borrow;
use std::fmt;
use std::path::PathBuf;

use crate::options::OptionDeclaration;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PatchId(Box<str>);

impl PatchId {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PatchId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for PatchId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for PatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for PatchId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PatchId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Box<str>> for PatchId {
    fn from(value: Box<str>) -> Self {
        Self(value)
    }
}

impl From<PatchId> for String {
    fn from(value: PatchId) -> Self {
        value.0.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilitySpec {
    pub package: Box<str>,
    pub versions: Vec<Box<str>>,
}

impl CompatibilitySpec {
    pub fn package(package: impl Into<Box<str>>) -> Self {
        Self {
            package: package.into(),
            versions: Vec::new(),
        }
    }

    pub fn with_versions<I, S>(package: impl Into<Box<str>>, versions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        Self {
            package: package.into(),
            versions: versions.into_iter().map(Into::into).collect(),
        }
    }

    pub fn matches(&self, package: &str, version: Option<&str>) -> bool {
        self.package.as_ref() == package
            && (self.versions.is_empty()
                || version.is_some_and(|version| {
                    self.versions
                        .iter()
                        .any(|allowed| allowed.as_ref() == version)
                }))
    }
}

#[derive(Debug, Clone)]
pub struct PatchSpec {
    pub id: PatchId,
    pub description: Box<str>,
    pub enabled_by_default: bool,
    pub dependencies: Vec<PatchId>,
    pub compatibility: Vec<CompatibilitySpec>,
    pub options: Vec<OptionDeclaration>,
    pub extension_dex: Vec<PathBuf>,
}

impl PatchSpec {
    pub fn new(id: impl Into<PatchId>) -> Self {
        Self {
            id: id.into(),
            description: "".into(),
            enabled_by_default: true,
            dependencies: Vec::new(),
            compatibility: Vec::new(),
            options: Vec::new(),
            extension_dex: Vec::new(),
        }
    }

    pub fn compatibility_reason(
        &self,
        package: Option<&str>,
        version: Option<&str>,
    ) -> Option<String> {
        if self.compatibility.is_empty() {
            return None;
        }

        let package = match package {
            Some(package) => package,
            None => return Some("APK has no package name".to_owned()),
        };

        let Some(entry) = self
            .compatibility
            .iter()
            .find(|entry| entry.package.as_ref() == package)
        else {
            return Some(format!("incompatible package: {package}"));
        };

        if entry.versions.is_empty() {
            return None;
        }

        match version {
            Some(version) if entry.matches(package, Some(version)) => None,
            Some(version) => Some(format!(
                "expected one of [{}], got {version}",
                entry
                    .versions
                    .iter()
                    .map(|allowed| allowed.as_ref())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            None => Some("APK has no version name".to_owned()),
        }
    }

    pub fn is_compatible(&self, package: Option<&str>, version: Option<&str>) -> bool {
        self.compatibility_reason(package, version).is_none()
    }
}
