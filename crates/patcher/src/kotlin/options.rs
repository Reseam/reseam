// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The running patch's option values.

use boltffi::export;

use super::handles::with_ctx;
use crate::options::OptionValue;

fn option<R>(key: &str, f: impl FnOnce(&OptionValue) -> Option<R>) -> Option<R> {
    with_ctx(|ctx| f(ctx.options().get(key)?))
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
    with_ctx(|ctx| ctx.options().list_path_contents(&key).ok().flatten())
}

#[export]
pub fn option_read_path_file(key: String, relative_path: String) -> Option<Vec<u8>> {
    with_ctx(|ctx| {
        ctx.options()
            .read_path_file(&key, &relative_path)
            .ok()
            .flatten()
    })
}
