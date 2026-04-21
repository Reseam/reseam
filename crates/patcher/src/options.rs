// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{PatcherError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OptionKey(Box<str>);

impl OptionKey {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for OptionKey {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for OptionKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for OptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for OptionKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for OptionKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Box<str>> for OptionKey {
    fn from(value: Box<str>) -> Self {
        Self(value)
    }
}

impl From<OptionKey> for String {
    fn from(value: OptionKey) -> Self {
        value.0.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionType {
    String,
    Bool,
    Int,
    Float,
    StringList,
    Path,
}

#[derive(Debug, Clone)]
pub struct OptionDeclaration {
    pub key: OptionKey,
    pub title: String,
    pub description: String,
    pub option_type: OptionType,
    pub default_value: Option<OptionValue>,
    pub valid_values: Option<Vec<String>>,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub enum OptionValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    StringList(Vec<String>),
    Path(PathBuf),
}

impl OptionValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            OptionValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            OptionValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            OptionValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            OptionValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            OptionValue::StringList(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            OptionValue::Path(p) => Some(p),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            OptionValue::String(_) => "string",
            OptionValue::Bool(_) => "bool",
            OptionValue::Int(_) => "int",
            OptionValue::Float(_) => "float",
            OptionValue::StringList(_) => "string list",
            OptionValue::Path(_) => "path",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PatchOptions {
    values: HashMap<OptionKey, OptionValue>,
}

impl PatchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<OptionKey>, value: OptionValue) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.values.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OptionKey, &OptionValue)> {
        self.values.iter()
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_int())
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(|v| v.as_float())
    }

    pub fn get_string_list(&self, key: &str) -> Option<&[String]> {
        self.values.get(key).and_then(|v| v.as_string_list())
    }

    pub fn get_path(&self, key: &str) -> Option<&Path> {
        self.values.get(key).and_then(|v| v.as_path())
    }

    pub fn list_path_contents(&self, key: &str) -> Result<Option<Vec<String>>> {
        let path = match self.get_path(key) {
            Some(p) => p,
            None => return Ok(None),
        };
        if !path.is_dir() {
            return Err(PatcherError::NotFound(format!(
                "option '{}' path is not a directory: {}",
                key,
                path.display()
            )));
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
        entries.sort();
        Ok(Some(entries))
    }

    pub fn read_path_file(&self, key: &str, relative: &str) -> Result<Option<Vec<u8>>> {
        let base = match self.get_path(key) {
            Some(p) => p,
            None => return Ok(None),
        };
        let base = base.canonicalize()?;
        let full = base.join(relative);
        let full = full.canonicalize().map_err(|e| {
            PatcherError::NotFound(format!("file not found: {} ({e})", full.display()))
        })?;
        if !full.starts_with(&base) {
            return Err(PatcherError::InvalidOptionValue {
                patch: String::new(),
                key: key.to_string(),
                reason: format!(
                    "path escapes option root: {} is outside {}",
                    full.display(),
                    base.display()
                ),
            });
        }
        if !full.exists() {
            return Err(PatcherError::NotFound(format!(
                "file not found: {}",
                full.display()
            )));
        }
        Ok(Some(std::fs::read(&full)?))
    }
}

impl OptionDeclaration {
    pub fn parse_value(&self, raw: &str) -> Result<OptionValue> {
        let value = match self.option_type {
            OptionType::String => OptionValue::String(raw.to_string()),
            OptionType::Bool => OptionValue::Bool(raw.parse::<bool>().map_err(|_| {
                PatcherError::InvalidOptionValue {
                    patch: String::new(),
                    key: self.key.to_string(),
                    reason: format!("expected bool, got '{raw}'"),
                }
            })?),
            OptionType::Int => OptionValue::Int(raw.parse::<i64>().map_err(|_| {
                PatcherError::InvalidOptionValue {
                    patch: String::new(),
                    key: self.key.to_string(),
                    reason: format!("expected int, got '{raw}'"),
                }
            })?),
            OptionType::Float => OptionValue::Float(raw.parse::<f64>().map_err(|_| {
                PatcherError::InvalidOptionValue {
                    patch: String::new(),
                    key: self.key.to_string(),
                    reason: format!("expected float, got '{raw}'"),
                }
            })?),
            OptionType::StringList => OptionValue::StringList(
                raw.split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
            ),
            OptionType::Path => OptionValue::Path(PathBuf::from(raw)),
        };
        self.validate_value(&value)?;
        Ok(value)
    }

    pub fn validate_value(&self, value: &OptionValue) -> Result<()> {
        let type_matches = matches!(
            (&self.option_type, value),
            (OptionType::String, OptionValue::String(_))
                | (OptionType::Bool, OptionValue::Bool(_))
                | (OptionType::Int, OptionValue::Int(_))
                | (OptionType::Float, OptionValue::Float(_))
                | (OptionType::StringList, OptionValue::StringList(_))
                | (OptionType::Path, OptionValue::Path(_))
        );

        if !type_matches {
            return Err(PatcherError::InvalidOptionValue {
                patch: String::new(),
                key: self.key.to_string(),
                reason: format!("expected {:?}, got {}", self.option_type, value.type_name()),
            });
        }

        if let Some(valid_values) = &self.valid_values {
            let values_to_check: Vec<&str> = match value {
                OptionValue::String(v) => vec![v.as_str()],
                OptionValue::StringList(v) => v.iter().map(String::as_str).collect(),
                _ => Vec::new(),
            };

            if !values_to_check.is_empty() {
                for candidate in values_to_check {
                    if !valid_values.iter().any(|allowed| allowed == candidate) {
                        return Err(PatcherError::InvalidOptionValue {
                            patch: String::new(),
                            key: self.key.to_string(),
                            reason: format!(
                                "'{candidate}' is not in [{}]",
                                valid_values.join(", ")
                            ),
                        });
                    }
                }
            }
        }

        if let OptionValue::Path(path) = value {
            if !path.exists() {
                return Err(PatcherError::InvalidOptionValue {
                    patch: String::new(),
                    key: self.key.to_string(),
                    reason: format!("path does not exist: {}", path.display()),
                });
            }
        }

        Ok(())
    }
}

pub fn validate_patch_options(
    patch_name: &str,
    declarations: &[OptionDeclaration],
    provided: Option<&PatchOptions>,
) -> Result<PatchOptions> {
    let mut resolved = PatchOptions::new();
    let provided = provided.cloned().unwrap_or_default();

    for (key, _) in provided.iter() {
        if !declarations.iter().any(|decl| decl.key == *key) {
            return Err(PatcherError::UnknownOption {
                patch: patch_name.to_string(),
                key: key.to_string(),
            });
        }
    }

    for decl in declarations {
        if let Some(value) = provided.get(decl.key.as_str()) {
            decl.validate_value(value).map_err(|err| match err {
                PatcherError::InvalidOptionValue { reason, .. } => {
                    PatcherError::InvalidOptionValue {
                        patch: patch_name.to_string(),
                        key: decl.key.to_string(),
                        reason,
                    }
                }
                other => other,
            })?;
            resolved.set(decl.key.clone(), value.clone());
            continue;
        }

        if let Some(default_value) = &decl.default_value {
            decl.validate_value(default_value)
                .map_err(|err| match err {
                    PatcherError::InvalidOptionValue { reason, .. } => {
                        PatcherError::InvalidOptionValue {
                            patch: patch_name.to_string(),
                            key: decl.key.to_string(),
                            reason,
                        }
                    }
                    other => other,
                })?;
            resolved.set(decl.key.clone(), default_value.clone());
            continue;
        }

        if decl.required {
            return Err(PatcherError::MissingRequiredOption {
                patch: patch_name.to_string(),
                key: decl.key.to_string(),
            });
        }
    }

    Ok(resolved)
}
