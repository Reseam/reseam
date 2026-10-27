// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod annotations;
pub(crate) mod assembly;
pub(crate) mod class_ops;
pub(crate) mod code;
pub(crate) mod fields;
pub(crate) mod lookup;
pub(crate) mod methods;
pub(crate) mod mutation;
pub(crate) mod pools;
pub(crate) mod positions;
pub(crate) mod registers;
pub(crate) mod replace_operands;
pub(crate) mod search;

fn logged<T>(what: &str, result: Result<T, impl std::fmt::Display>) -> Result<T, String> {
    result.map_err(|error| format!("{what} failed: {error}"))
}

#[cfg(test)]
mod tests;
