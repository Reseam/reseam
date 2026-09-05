// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PatcherError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    String,
    Bool,
    Int,
    Float,
    StringList,
    Path,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionDeclaration {
    pub key: String,
    pub title: String,
    pub description: String,
    pub option_type: OptionType,
    pub default_value: Option<OptionValue>,
    pub valid_values: Option<Vec<String>>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum OptionValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    StringList(Vec<String>),
    Path(PathBuf),
}

impl OptionValue {
    pub fn option_type(&self) -> OptionType {
        match self {
            Self::String(_) => OptionType::String,
            Self::Bool(_) => OptionType::Bool,
            Self::Int(_) => OptionType::Int,
            Self::Float(_) => OptionType::Float,
            Self::StringList(_) => OptionType::StringList,
            Self::Path(_) => OptionType::Path,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            Self::StringList(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(p) => Some(p),
            _ => None,
        }
    }
}

impl OptionDeclaration {
    pub fn parse(&self, raw: &str) -> std::result::Result<OptionValue, String> {
        let value = match self.option_type {
            OptionType::String => OptionValue::String(raw.to_string()),
            OptionType::Bool => OptionValue::Bool(
                raw.parse()
                    .map_err(|_| format!("expected bool, got '{raw}'"))?,
            ),
            OptionType::Int => OptionValue::Int(
                raw.parse()
                    .map_err(|_| format!("expected int, got '{raw}'"))?,
            ),
            OptionType::Float => OptionValue::Float(
                raw.parse()
                    .map_err(|_| format!("expected float, got '{raw}'"))?,
            ),
            OptionType::StringList => OptionValue::StringList(
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            OptionType::Path => OptionValue::Path(PathBuf::from(raw)),
        };
        self.validate(&value)?;
        Ok(value)
    }

    pub fn validate(&self, value: &OptionValue) -> std::result::Result<(), String> {
        if value.option_type() != self.option_type {
            return Err(format!(
                "expected {:?}, got {:?}",
                self.option_type,
                value.option_type()
            ));
        }
        if let Some(valid) = &self.valid_values {
            let candidates: &[String] = match value {
                OptionValue::String(s) => std::slice::from_ref(s),
                OptionValue::StringList(list) => list,
                _ => &[],
            };
            if let Some(bad) = candidates
                .iter()
                .find(|candidate| !valid.contains(candidate))
            {
                return Err(format!("'{bad}' is not in [{}]", valid.join(", ")));
            }
        }
        if let OptionValue::Path(path) = value {
            if !path.exists() {
                return Err(format!("path does not exist: {}", path.display()));
            }
        }
        Ok(())
    }
}

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
