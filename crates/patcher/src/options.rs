// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{PatcherError, Result};

pub use reseam_model::{OptionDeclaration, OptionType, OptionValue};

/// The option values one patch runs with.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct PatchOptions {
    values: HashMap<String, OptionValue>,
}

impl PatchOptions {
    pub fn set(&mut self, key: impl Into<String>, value: OptionValue) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.values.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue)> {
        self.values.iter().map(|(key, value)| (key.as_str(), value))
    }

    /// Checks `provided` against `declarations` and fills in defaults.
    pub fn resolve(
        patch: &str,
        declarations: &[OptionDeclaration],
        provided: Option<&Self>,
    ) -> Result<Self> {
        let provided = provided.cloned().unwrap_or_default();
        if let Some(key) = provided
            .values
            .keys()
            .find(|key| !declarations.iter().any(|decl| decl.key == **key))
        {
            return Err(PatcherError::UnknownOption {
                patch: patch.to_string(),
                key: key.clone(),
            });
        }
        let mut resolved = Self::default();
        for decl in declarations {
            let value = match provided.get(&decl.key).or(decl.default_value.as_ref()) {
                Some(value) => value,
                None if decl.required => {
                    return Err(PatcherError::MissingRequiredOption {
                        patch: patch.to_string(),
                        key: decl.key.clone(),
                    })
                }
                None => continue,
            };
            decl.validate(value)
                .map_err(|reason| PatcherError::InvalidOptionValue {
                    patch: patch.to_string(),
                    key: decl.key.clone(),
                    reason,
                })?;
            resolved.set(decl.key.clone(), value.clone());
        }
        Ok(resolved)
    }

    /// Sorted file names inside a `Path` option's directory.
    pub fn list_path_contents(&self, key: &str) -> Result<Option<Vec<String>>> {
        let Some(path) = self.get(key).and_then(OptionValue::as_path) else {
            return Ok(None);
        };
        if !path.is_dir() {
            return Err(PatcherError::NotFound(format!(
                "option '{key}' path is not a directory: {}",
                path.display()
            )));
        }
        let mut entries: Vec<String> = std::fs::read_dir(path)?
            .filter_map(|entry| entry.ok()?.file_name().to_str().map(str::to_string))
            .collect();
        entries.sort();
        Ok(Some(entries))
    }

    /// A file under a `Path` option's directory; paths escaping it are refused.
    pub fn read_path_file(&self, key: &str, relative: &str) -> Result<Option<Vec<u8>>> {
        let Some(base) = self.get(key).and_then(OptionValue::as_path) else {
            return Ok(None);
        };
        let base = base.canonicalize()?;
        let full = base.join(relative);
        let full = full.canonicalize().map_err(|e| {
            PatcherError::NotFound(format!("file not found: {} ({e})", full.display()))
        })?;
        if !full.starts_with(&base) {
            return Err(PatcherError::NotFound(format!(
                "option '{key}': {} is outside {}",
                full.display(),
                base.display()
            )));
        }
        Ok(Some(std::fs::read(&full)?))
    }
}

impl From<HashMap<String, OptionValue>> for PatchOptions {
    fn from(values: HashMap<String, OptionValue>) -> Self {
        Self { values }
    }
}
