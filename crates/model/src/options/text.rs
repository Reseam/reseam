// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The text form of option values, as users type them: `true` or `false`, a
//! whole or decimal number, one list item per line, and anything else as is.

use std::fmt;

use super::{OptionDeclaration, OptionProblem, OptionType, OptionValue};

impl OptionDeclaration {
    /// Reads `text` as a value of this option and checks it against the
    /// declared choices. Numbers may have surrounding whitespace; list items are
    /// trimmed and blank lines left out.
    pub fn parse(&self, text: &str) -> Result<OptionValue, OptionProblem> {
        let value = match self.option_type {
            OptionType::String => OptionValue::Text(text.to_owned()),
            OptionType::Path => OptionValue::Path(text.to_owned()),
            OptionType::Bool => match text.trim() {
                "true" => OptionValue::Bool(true),
                "false" => OptionValue::Bool(false),
                _ => return Err(OptionProblem::NotABool),
            },
            OptionType::Int => OptionValue::Int(
                text.trim()
                    .parse()
                    .map_err(|_| OptionProblem::NotAnInteger)?,
            ),
            OptionType::Float => OptionValue::Float(
                text.trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or(OptionProblem::NotANumber)?,
            ),
            OptionType::StringList => OptionValue::TextList(
                text.lines()
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
        };
        self.check(&value)?;
        Ok(value)
    }
}

/// The text [`OptionDeclaration::parse`] reads back as the same value.
impl fmt::Display for OptionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) | Self::Path(text) => f.write_str(text),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::TextList(items) => f.write_str(&items.join("\n")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(option_type: OptionType, choices: Option<&[&str]>) -> OptionDeclaration {
        OptionDeclaration {
            key: "key".to_owned(),
            title: "Key".to_owned(),
            description: String::new(),
            option_type,
            default_value: None,
            valid_values: choices.map(|values| values.iter().map(|&v| v.to_owned()).collect()),
            required: false,
        }
    }

    #[test]
    fn text_reads_as_the_declared_type_and_prints_back_the_same() {
        let list =
            |items: &[&str]| OptionValue::TextList(items.iter().map(|&i| i.to_owned()).collect());
        for (option_type, choices, text, expected) in [
            (
                OptionType::String,
                None,
                " a b ",
                Ok(OptionValue::Text(" a b ".to_owned())),
            ),
            (OptionType::Bool, None, "true", Ok(OptionValue::Bool(true))),
            (OptionType::Bool, None, "yes", Err(OptionProblem::NotABool)),
            (OptionType::Int, None, " -42 ", Ok(OptionValue::Int(-42))),
            (
                OptionType::Int,
                None,
                "9223372036854775808",
                Err(OptionProblem::NotAnInteger),
            ),
            (
                OptionType::Int,
                None,
                "1.5",
                Err(OptionProblem::NotAnInteger),
            ),
            (OptionType::Float, None, "1.5", Ok(OptionValue::Float(1.5))),
            (
                OptionType::Float,
                None,
                "inf",
                Err(OptionProblem::NotANumber),
            ),
            (
                OptionType::StringList,
                None,
                "a\n\n b \n",
                Ok(list(&["a", "b"])),
            ),
            (
                OptionType::StringList,
                Some(&["a", "b"][..]),
                "a\nc",
                Err(OptionProblem::NotAllowed {
                    value: "c".to_owned(),
                    allowed: vec!["a".to_owned(), "b".to_owned()],
                }),
            ),
            (
                OptionType::String,
                Some(&["home"][..]),
                "home",
                Ok(OptionValue::Text("home".to_owned())),
            ),
            (
                OptionType::Path,
                None,
                "/data",
                Ok(OptionValue::Path("/data".to_owned())),
            ),
        ] {
            let declaration = declaration(option_type, choices);
            let parsed = declaration.parse(text);
            assert_eq!(parsed, expected, "{option_type:?} {text:?}");
            if let Ok(value) = parsed {
                assert_eq!(declaration.parse(&value.to_string()), Ok(value));
            }
        }
    }

    #[test]
    fn a_required_option_needs_a_value_or_a_default() {
        let mut required = declaration(OptionType::Int, None);
        required.required = true;
        assert_eq!(required.resolve(None), Err(OptionProblem::Missing));
        assert_eq!(
            required.resolve(Some(&OptionValue::Bool(true))),
            Err(OptionProblem::WrongType {
                expected: OptionType::Int,
                actual: OptionType::Bool,
            })
        );
        required.default_value = Some(OptionValue::Int(1));
        assert_eq!(required.resolve(None), Ok(Some(&OptionValue::Int(1))));
        assert_eq!(declaration(OptionType::Int, None).resolve(None), Ok(None));
    }
}
