// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Kotlin patch runtime: patches are JVM objects that call back into
//! the engine through the `#[export]` functions in this module tree.

#[cfg(target_os = "android")]
pub mod android_host;
mod bytecode;
mod convert;
mod files;
mod handles;
pub(crate) mod jvm;
mod loader;
mod log_host;
mod manifest;
mod options;
mod patch;
mod resources;
pub mod types;
mod xml;

use boltffi::export;

pub use loader::load_patches;

#[export]
pub fn ctx_is_active() -> bool {
    handles::context_is_active()
}

#[export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
