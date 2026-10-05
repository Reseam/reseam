// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::needless_pass_by_value,
    reason = "BoltFFI exports own arguments decoded across JNI"
)]

#[cfg(target_os = "android")]
pub mod android_host;
mod bytecode;
mod convert;
mod files;
mod handle_table;
pub(crate) mod handles;
mod hermes;
mod instruction_types;
mod invoke;
#[cfg(feature = "kotlin")]
pub(crate) mod jvm;
mod link;
#[cfg(feature = "kotlin")]
mod loader;
mod log_host;
mod manifest;
#[cfg(feature = "kotlin")]
mod metadata;
mod options;
#[cfg(feature = "kotlin")]
mod patch;
mod pool_values;
mod resource_files;
mod resources;
pub mod types;
mod xml;
mod xml_attributes;
mod xml_documents;
mod xml_nodes;

use boltffi::export;

#[cfg(feature = "kotlin")]
pub use loader::load_patches;
#[cfg(all(feature = "browser", not(feature = "kotlin")))]
mod browser;
#[cfg(all(feature = "browser", not(feature = "kotlin")))]
pub use browser::load_patches;

#[export]
pub fn ctx_is_active() -> bool {
    handles::context_is_active()
}
