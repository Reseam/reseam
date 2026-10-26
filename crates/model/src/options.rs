// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[boltffi::data]
pub enum OptionValue {
    #[serde(rename = "string")]
    Text(String),
    Bool(bool),
    /// A Java long; values beyond 2^53 cross JSON hosts as `bigint`.
    Int(#[cfg_attr(feature = "typescript", ts(type = "number | bigint"))] i64),
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
