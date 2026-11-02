// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{Compatibility, OptionDeclaration, Problem};
use serde::{Deserialize, Serialize};

/// A bulk selection clients offer next to toggling patches one by one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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

/// A patch's identity: the bundle that declares it and its id there. It
/// serializes as its text form `<bundle>/<id>`, the form published release
/// indexes and bundle manifests store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
#[boltffi::data]
pub struct PatchRef {
    pub bundle: String,
    /// `<package>.<property>` of the public declaration; unique within the bundle.
    pub id: String,
}

impl PatchRef {
    /// The reference `<bundle>/<id>` names, if `text` has that form.
    pub fn parse(text: &str) -> Option<Self> {
        let (bundle, id) = text.split_once('/')?;
        Some(Self {
            bundle: bundle.to_owned(),
            id: id.to_owned(),
        })
    }
}

/// The schema of the text form `PatchRef` serializes as.
#[cfg(feature = "schema")]
impl schemars::JsonSchema for PatchRef {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PatchRef".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A patch as `<bundle>/<id>`.",
        })
    }
}

impl std::fmt::Display for PatchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.bundle, self.id)
    }
}

impl Serialize for PatchRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PatchRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("patch reference {text:?} is not <bundle>/<id>"))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
    /// The patches that run first.
    pub dependencies: Vec<PatchRef>,
    pub compatibility: Compatibility,
    pub options: Vec<OptionDeclaration>,
    /// Replaces the APK files the engine writes, before they are signed. A mount
    /// build leaves such a patch out, since it cannot tell what the files hold.
    #[serde(default)]
    #[boltffi::default(false)]
    pub rewrites_output: bool,
}

impl PatchSpec {
    pub fn reference(&self) -> PatchRef {
        PatchRef {
            bundle: self.bundle.clone(),
            id: self.id.clone(),
        }
    }

    /// Whether `preset` selects the patch for an app of `package`. Presets take
    /// the patches that declare the package, and a universal patch only when its
    /// author enables it by default. Version compatibility is not considered; it
    /// is reported separately.
    pub fn in_preset(&self, preset: PatchPreset, package: Option<&str>) -> bool {
        let declared = self.compatibility.declares(package);
        let universal = self.compatibility.is_universal();
        !self.hidden
            && match preset {
                PatchPreset::Recommended => (declared || universal) && self.enabled_by_default,
                PatchPreset::All => declared || universal && self.enabled_by_default,
                PatchPreset::None => false,
            }
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
            rewrites_output: false,
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
            [crate::CompatiblePackage {
                package: "com.example".to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
                version_codes: Vec::new(),
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

#[cfg(all(test, feature = "schema"))]
mod schema_tests {
    #[test]
    fn a_patch_reference_is_a_string_in_json_schema() {
        let schema = schemars::schema_for!(super::PatchRef);
        assert_eq!(schema.get("type"), Some(&serde_json::json!("string")));
    }
}
