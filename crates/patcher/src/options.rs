// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{PatcherError, Result};
use crate::patch::PatchRef;

use reseam_model::OptionProblem;
pub use reseam_model::{OptionDeclaration, OptionType, OptionValue};

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

    /// What is wrong with `provided` for `declarations`: keys no declaration
    /// has, then declarations whose value is missing or does not fit.
    pub fn problems(
        declarations: &[OptionDeclaration],
        provided: Option<&Self>,
    ) -> Vec<(String, OptionProblem)> {
        let undeclared = provided
            .into_iter()
            .flat_map(Self::iter)
            .filter(|(key, _)| !declarations.iter().any(|decl| decl.key == *key))
            .map(|(key, _)| (key.to_owned(), OptionProblem::Undeclared));
        let invalid = declarations.iter().filter_map(|decl| {
            decl.resolve(provided.and_then(|options| options.get(&decl.key)))
                .err()
                .map(|problem| (decl.key.clone(), problem))
        });
        undeclared.chain(invalid).collect()
    }

    /// Checks `provided` against `declarations` and fills in defaults. Path
    /// values must also be accessible.
    pub fn resolve(
        patch: &PatchRef,
        declarations: &[OptionDeclaration],
        provided: Option<&Self>,
    ) -> Result<Self> {
        if let Some((key, problem)) = Self::problems(declarations, provided).into_iter().next() {
            return Err(match problem {
                OptionProblem::Undeclared => PatcherError::UnknownOption {
                    patch: patch.clone(),
                    key,
                },
                OptionProblem::Missing => PatcherError::MissingRequiredOption {
                    patch: patch.clone(),
                    key,
                },
                problem => PatcherError::InvalidOptionValue {
                    patch: patch.clone(),
                    key,
                    reason: problem.to_string(),
                },
            });
        }
        let mut resolved = Self::default();
        for decl in declarations {
            let Some(value) = decl
                .resolve(provided.and_then(|options| options.get(&decl.key)))
                .expect("problems found every option that does not resolve")
            else {
                continue;
            };
            if let OptionValue::Path(path) = value {
                std::fs::metadata(path).map_err(|error| PatcherError::InvalidOptionValue {
                    patch: patch.clone(),
                    key: decl.key.clone(),
                    reason: format!("cannot access path {path}: {error}"),
                })?;
            }
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
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let name = entry?.file_name();
            if let Some(name) = name.to_str() {
                entries.push(name.to_owned());
            }
        }
        entries.sort();
        Ok(Some(entries))
    }

    /// A file under a `Path` option's directory; paths escaping it are refused.
    pub fn read_path_file(&self, key: &str, relative: &str) -> Result<Option<Vec<u8>>> {
        let Some(base) = self.get(key).and_then(OptionValue::as_path) else {
            return Ok(None);
        };
        let base = reseam_storage::canonicalize(base)?;
        let full = base.join(relative);
        let full = reseam_storage::canonicalize(&full).map_err(|e| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_must_match_the_declared_type_choices_and_an_accessible_path() {
        let declaration = |option_type, valid_values: Option<&[&str]>| OptionDeclaration {
            key: "targets".into(),
            title: "Targets".into(),
            description: String::new(),
            option_type,
            default_value: None,
            valid_values: valid_values.map(|values| values.iter().map(|&v| v.into()).collect()),
            required: true,
        };
        let choices = Some(["home", "feed"].as_slice());
        let list =
            |values: &[&str]| OptionValue::TextList(values.iter().map(|&v| v.into()).collect());
        let existing = std::env::temp_dir().display().to_string();
        for (declaration, value, accepted) in [
            (
                declaration(OptionType::StringList, choices),
                list(&["feed"]),
                true,
            ),
            (
                declaration(OptionType::StringList, choices),
                list(&["other"]),
                false,
            ),
            (
                declaration(OptionType::String, choices),
                OptionValue::Text("home".into()),
                true,
            ),
            (
                declaration(OptionType::StringList, choices),
                OptionValue::Bool(true),
                false,
            ),
            (
                declaration(OptionType::Path, None),
                OptionValue::Path(existing),
                true,
            ),
            (
                declaration(OptionType::Path, None),
                OptionValue::Path("/no/such/path".into()),
                false,
            ),
        ] {
            let mut provided = PatchOptions::default();
            provided.set("targets", value.clone());
            let resolved = PatchOptions::resolve(
                &PatchRef::parse("bundle/patch").unwrap(),
                &[declaration],
                Some(&provided),
            );
            match resolved {
                Ok(options) => {
                    assert!(accepted, "{value:?}");
                    assert_eq!(options.get("targets"), Some(&value));
                }
                Err(error) => assert!(
                    !accepted && matches!(error, PatcherError::InvalidOptionValue { .. }),
                    "{value:?}: {error}"
                ),
            }
        }
    }
}
