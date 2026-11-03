// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Host-facing API for inspecting APKs and bundles and running typed patch requests.
//! The request, response and error types are in `reseam-model`.

pub mod analysis;
mod apk;
mod apply;
mod archive;
pub mod authoring;
mod bundles;
mod changes;
mod error;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
mod inputs;
mod inspection;
mod install;
mod memory;
mod metrics;
pub mod mount;
mod output;
mod plan;
mod progress;
mod publish;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
mod resolve;
mod run;
mod signing;
mod staging;
mod trust;

pub use apk::ApkInspection;
pub use archive::write_split_archive;
pub use changes::changes;
pub use error::HostError;
pub use inspection::{PreparedInspection, inspect};
pub use install::dex_metadata;
pub use plan::plan;
#[cfg(target_os = "android")]
pub use reseam_patcher::bridge::runtime::android_host::install_class_loader;
pub use reseam_patcher::bundle::ENGINE_VERSION;
pub use reseam_patcher::error::PatcherError;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub use resolve::resolve;
pub use run::patch;
pub use signing::{certificate_fingerprint, export_pkcs12, import_pkcs12};

#[cfg(test)]
mod tests;
