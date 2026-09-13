// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum OptionType {
    String,
    Bool,
    Int,
    Float,
    StringList,
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
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
#[boltffi::data]
pub enum OptionValue {
    #[serde(rename = "string")]
    Text(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    #[serde(rename = "string_list")]
    TextList(Vec<String>),
    Path(String),
}

impl OptionValue {
    pub fn option_type(&self) -> OptionType {
        match self {
            Self::Text(_) => OptionType::String,
            Self::Bool(_) => OptionType::Bool,
            Self::Int(_) => OptionType::Int,
            Self::Float(_) => OptionType::Float,
            Self::TextList(_) => OptionType::StringList,
            Self::Path(_) => OptionType::Path,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
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
            Self::TextList(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(p) => Some(Path::new(p)),
            _ => None,
        }
    }
}

impl OptionDeclaration {
    pub fn parse(&self, raw: &str) -> std::result::Result<OptionValue, String> {
        let value = match self.option_type {
            OptionType::String => OptionValue::Text(raw.to_string()),
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
            OptionType::StringList => OptionValue::TextList(
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            OptionType::Path => OptionValue::Path(raw.to_owned()),
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
                OptionValue::Text(s) => std::slice::from_ref(s),
                OptionValue::TextList(list) => list,
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
            if !Path::new(path).exists() {
                return Err(format!("path does not exist: {}", path));
            }
        }
        Ok(())
    }
}
