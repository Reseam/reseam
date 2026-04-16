// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod class_ops;
mod lookup;
mod mutation;
mod registers;
mod search;

use boltffi::export;

#[export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
