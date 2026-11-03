// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod check;
mod text;

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

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

/// Why a value does not fit an option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum OptionProblem {
    /// The option is required, has no default, and was given no value.
    #[error("needs a value")]
    Missing,
    /// The patch declares no option with this key.
    #[error("the patch has no such option")]
    Undeclared,
    #[error("expected {}, got {}", expected.as_str(), actual.as_str())]
    WrongType {
        expected: OptionType,
        actual: OptionType,
    },
    #[error("expected true or false")]
    NotABool,
    /// Not a whole number a Java long holds.
    #[error("expected a whole number from -9223372036854775808 to 9223372036854775807")]
    NotAnInteger,
    /// Not a finite decimal number.
    #[error("expected a number")]
    NotANumber,
    /// `value` is not among the choices the option declares.
    #[error("'{value}' is not one of {}", allowed.join(", "))]
    NotAllowed { value: String, allowed: Vec<String> },
}

/// What reading option text gives: the value, or why the text is not one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParsedOption {
    Value { value: OptionValue },
    Problem { problem: OptionProblem },
}

impl OptionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::StringList => "string_list",
            Self::Path => "path",
        }
    }
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
