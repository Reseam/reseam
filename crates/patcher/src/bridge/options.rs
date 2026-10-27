// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;

use crate::bridge::phase::with_patch;
use crate::bridge::run_state::checked;
use crate::error::Result;
use crate::options::{OptionValue, PatchOptions};

fn option<R>(key: &str, f: impl FnOnce(&OptionValue) -> Option<R>) -> Option<R> {
    with_patch(|scope| f(scope.options.get(key)?))
}

fn file_option<R>(key: &str, f: impl FnOnce(&PatchOptions) -> Result<Option<R>>) -> Option<R> {
    with_patch(|scope| {
        checked(f(&scope.options).map_err(|error| format!("option '{key}': {error}")))
    })
}

#[export]
pub fn option_get_string(key: String) -> Option<String> {
    option(&key, |v| v.as_str().map(str::to_string))
}

#[export]
pub fn option_get_bool(key: String) -> Option<bool> {
    option(&key, OptionValue::as_bool)
}

#[export]
pub fn option_get_int(key: String) -> Option<i64> {
    option(&key, OptionValue::as_int)
}

#[export]
pub fn option_get_float(key: String) -> Option<f64> {
    option(&key, OptionValue::as_float)
}

#[export]
pub fn option_get_string_list(key: String) -> Option<Vec<String>> {
    option(&key, |v| v.as_string_list().map(<[String]>::to_vec))
}

#[export]
pub fn option_get_path(key: String) -> Option<String> {
    option(&key, |v| {
        v.as_path().map(|p| p.to_string_lossy().into_owned())
    })
}

#[export]
pub fn option_list_path_contents(key: String) -> Option<Vec<String>> {
    file_option(&key, |options| options.list_path_contents(&key))
}

#[export]
pub fn option_read_path_file(key: String, relative_path: String) -> Option<Vec<u8>> {
    file_option(&key, |options| options.read_path_file(&key, &relative_path))
}
