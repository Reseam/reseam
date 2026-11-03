// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{OptionDeclaration, OptionProblem, OptionValue};

impl OptionDeclaration {
    /// The value a run uses when the caller gave `provided`: it, or else the
    /// default, or nothing for an optional option without one.
    pub fn resolve<'a>(
        &'a self,
        provided: Option<&'a OptionValue>,
    ) -> Result<Option<&'a OptionValue>, OptionProblem> {
        let Some(value) = provided.or(self.default_value.as_ref()) else {
            return if self.required {
                Err(OptionProblem::Missing)
            } else {
                Ok(None)
            };
        };
        self.check(value)?;
        Ok(Some(value))
    }

    /// Whether `value` has this option's type and, when the option declares
    /// choices, is one of them; for a list, every item is.
    pub fn check(&self, value: &OptionValue) -> Result<(), OptionProblem> {
        if value.option_type() != self.option_type {
            return Err(OptionProblem::WrongType {
                expected: self.option_type,
                actual: value.option_type(),
            });
        }
        let Some(allowed) = &self.valid_values else {
            return Ok(());
        };
        let items = match value {
            OptionValue::Text(text) => std::slice::from_ref(text),
            OptionValue::TextList(items) => items,
            _ => &[],
        };
        match items.iter().find(|item| !allowed.contains(item)) {
            Some(item) => Err(OptionProblem::NotAllowed {
                value: item.clone(),
                allowed: allowed.clone(),
            }),
            None => Ok(()),
        }
    }
}
