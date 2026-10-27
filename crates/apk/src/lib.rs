// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod apk_file;
pub mod art_profile;
pub mod axml;
mod chunk;
pub mod compression;
mod container;
pub mod entry;
pub mod error;
pub mod resources;
pub use reseam_storage::{Bytes, ScratchDir};
mod res_value;
mod string_pool;
mod zip;

pub use apk_file::{
    ApkComponent, ApkFile, ApkWriteOptions, ApplicationIcon, DexSet, IconLayer, SignaturePolicy,
};
pub use axml::AxmlDocument;
pub use container::{ContainerFormat, open_container};
pub use error::{ApkError, Result};
pub use res_value::ResValue;
pub use resources::{ResourceScope, ResourceTable};
pub use string_pool::{StringEncoding, StringPool};
pub use zip::Compression;
