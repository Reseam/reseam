use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{PatcherError, Result};

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
    pub key: String,
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
}

#[derive(Debug, Clone, Default)]
pub struct PatchOptions {
    values: HashMap<String, OptionValue>,
}

impl PatchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: OptionValue) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.values.get(key)
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
        let full = base.join(relative);
        if !full.exists() {
            return Err(PatcherError::NotFound(format!(
                "file not found: {}",
                full.display()
            )));
        }
        Ok(Some(std::fs::read(&full)?))
    }
}
