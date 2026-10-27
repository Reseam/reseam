// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Host-facing API for inspecting APKs and bundles and running typed patch requests.
//! The request, response and error types are in `reseam-model`.

mod apk;
mod apply;
pub mod authoring;
mod bundles;
mod error;
mod inputs;
mod inspection;
mod install;
mod memory;
mod metrics;
mod output;
mod publish;
mod run;
mod trust;

pub use apk::ApkInspection;
pub use error::HostError;
pub use inspection::{PreparedInspection, inspect};
pub use install::dex_metadata;
#[cfg(target_os = "android")]
pub use reseam_patcher::bridge::runtime::android_host::install_class_loader;
pub use run::patch;

#[cfg(test)]
mod tests;
