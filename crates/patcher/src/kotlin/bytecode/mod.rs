// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod annotations;
mod assembly;
mod class_ops;
mod fields;
mod lookup;
mod methods;
mod mutation;
mod pools;
mod registers;
mod replace_operands;
mod search;

fn logged<T>(what: &str, result: Result<T, impl std::fmt::Display>) -> Result<T, String> {
    result.map_err(|error| format!("{what} failed: {error}"))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
