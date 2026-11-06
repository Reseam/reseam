// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The modules a bundle's payload is built from, which the Gradle plugin stages as
//! `modules.json` beside the payload (`StageBundleTask.kt`). Both sides check
//! `tests/fixtures/bundle-modules.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::stage::Staged;
use crate::error::{BundleError, Result};

pub(super) const MODULES: &str = "modules.json";

/// One module of a build: the payload files it makes and the modules it uses.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Module {
    pub(super) name: ModuleName,
    pub(super) files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) uses: Vec<ModuleName>,
    /// For files built from every module that uses them, such as a library shrunk to
    /// what all of them call: the SHA-256 of their other inputs. It stands for the
    /// files' content, since each user's part is in that user's own files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<Sha256>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(super) struct ModuleName(String);

/// A SHA-256 digest, written as lowercase hex.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Sha256(pub(super) [u8; 32]);

impl Serialize for Sha256 {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Sha256 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        hex::decode(&text)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom(format!("{text} is not a SHA-256 digest")))
    }
}

/// The payload's modules, each payload file in exactly one.
pub(super) struct Modules {
    modules: BTreeMap<ModuleName, Module>,
    owners: BTreeMap<String, ModuleName>,
}

impl Modules {
    /// Reads `dir/modules.json` and checks it against `payload`. A staging folder
    /// without one is a single module that makes the whole payload.
    pub(super) fn read(dir: &Path, payload: &BTreeMap<String, Staged>) -> Result<Self> {
        let path = dir.join(MODULES);
        let list: Vec<Module> = if path.exists() {
            serde_json::from_slice(&std::fs::read(&path)?).map_err(BundleError::Modules)?
        } else {
            vec![Module {
                name: ModuleName(String::new()),
                files: payload.keys().cloned().collect(),
                uses: Vec::new(),
                source: None,
            }]
        };
        let mut modules = BTreeMap::new();
        let mut owners = BTreeMap::new();
        for module in list {
            for file in &module.files {
                if !payload.contains_key(file) {
                    return Err(BundleError::ModuleFileMissing {
                        module: module.name.0.clone(),
                        file: file.clone(),
                    }
                    .into());
                }
                if owners.insert(file.clone(), module.name.clone()).is_some() {
                    return Err(BundleError::FileInModules(file.clone()).into());
                }
            }
            let name = module.name.clone();
            if modules.insert(name.clone(), module).is_some() {
                return Err(BundleError::ModuleTwice(name.0).into());
            }
        }
        if let Some(file) = payload.keys().find(|file| !owners.contains_key(*file)) {
            return Err(BundleError::FileInNoModule(file.clone()).into());
        }
        for module in modules.values() {
            if let Some(used) = module.uses.iter().find(|used| !modules.contains_key(*used)) {
                return Err(BundleError::UnknownModule {
                    module: module.name.0.clone(),
                    uses: used.0.clone(),
                }
                .into());
            }
        }
        Ok(Self { modules, owners })
    }

    /// The modules that make `files`, and every module they use, directly or not.
    pub(super) fn used_by<'f>(&self, files: impl IntoIterator<Item = &'f str>) -> Vec<&Module> {
        let mut pending: Vec<&ModuleName> = files
            .into_iter()
            .map(|file| {
                self.owners
                    .get(file)
                    .expect("every payload file has a module, as reading checks")
            })
            .collect();
        let mut seen = BTreeSet::new();
        let mut used = Vec::new();
        while let Some(name) = pending.pop() {
            if seen.insert(name) {
                let module = &self.modules[name];
                pending.extend(&module.uses);
                used.push(module);
            }
        }
        used
    }
}
