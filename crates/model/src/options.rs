// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Problem, Result};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Checks the value's type and declared choices without accessing the filesystem.
    /// Path existence is checked by the engine when resolving a run's options.
    pub fn validate(&self, value: &OptionValue) -> Result<()> {
        if value.option_type() != self.option_type {
            return Err(Problem::OptionType {
                key: self.key.clone(),
                expected: self.option_type,
                actual: value.option_type(),
            });
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
                return Err(Problem::OptionChoice {
                    key: self.key.clone(),
                    value: bad.clone(),
                    allowed: valid.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_obey_types_and_choices_without_environmental_validation() {
        let mut declaration = OptionDeclaration {
            key: "targets".into(),
            title: "Targets".into(),
            description: String::new(),
            option_type: OptionType::StringList,
            default_value: None,
            valid_values: Some(vec!["home".into(), "feed".into()]),
            required: true,
        };
        assert!(
            declaration
                .validate(&OptionValue::TextList(vec!["feed".into()]))
                .is_ok()
        );
        assert!(matches!(
            declaration.validate(&OptionValue::TextList(vec!["other".into()])),
            Err(Problem::OptionChoice { .. })
        ));
        let problem = declaration.validate(&OptionValue::Bool(true)).unwrap_err();
        assert!(matches!(problem, Problem::OptionType { .. }));
        let response = crate::SdkError {
            message: problem.to_string(),
            problem,
        };
        let json = serde_json::to_string(&response).unwrap();
        let decoded: crate::SdkError = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.problem, response.problem);
        declaration.option_type = OptionType::Path;
        declaration.valid_values = None;
        assert!(
            declaration
                .validate(&OptionValue::Path("/a/host-specific/path".into()))
                .is_ok()
        );
    }
}
