// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod apk_file;
pub mod axml;
mod buf;
mod chunk;
mod container;
mod dex;
pub mod entry;
pub mod error;
pub mod resources;
pub mod scratch;
mod string_pool;
mod value;
mod zip;

pub use apk_file::{
    ApkComponent, ApkFile, ApkWriteOptions, ApplicationIcon, Compression, IconLayer,
};
pub use axml::AxmlDocument;
pub use container::{ContainerBundle, ContainerFormat};
pub use dex::extract_dex;
pub use error::{ApkError, Result};
pub use resources::ResourceTable;
pub use string_pool::StringPool;
pub use value::ResValue;

pub use reseam_dex;
