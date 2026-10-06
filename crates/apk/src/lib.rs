// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod apk_file;
pub mod axml;
mod buf;
mod chunk;
pub mod compression;
mod container;
mod dex;
pub mod entry;
pub mod error;
pub mod resources;
pub use reseam_storage::{Bytes, ScratchDir};
mod string_pool;
mod value;
mod zip;

pub use apk_file::{
    ApkComponent, ApkFile, ApkWriteOptions, ApplicationIcon, Compression, IconLayer,
    SignaturePolicy,
};
pub use axml::AxmlDocument;
pub use container::{ContainerBundle, ContainerFormat};
pub use dex::extract_dex;
pub use error::{ApkError, Result};
pub use resources::{ResourceScope, ResourceTable};
pub use string_pool::{StringEncoding, StringPool};
pub use value::ResValue;

pub use reseam_dex;
