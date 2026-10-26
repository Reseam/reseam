// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
mod identity;
mod instruction_types;
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
mod output;
#[cfg(feature = "kotlin")]
mod patch;
mod pool_copies;
mod pool_values;
mod resource_files;
mod resources;
pub(crate) mod savepoints;
pub mod types;
mod xml;
mod xml_attributes;
mod xml_documents;
mod xml_nodes;

#[cfg(feature = "kotlin")]
pub(crate) use loader::load_patches;
#[cfg(all(feature = "browser", not(feature = "kotlin")))]
mod browser;
#[cfg(all(feature = "browser", not(feature = "kotlin")))]
pub(crate) use browser::load_patches;
