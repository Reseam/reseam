// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(target_os = "android")]
pub mod android_host;
#[cfg(all(feature = "browser", not(feature = "kotlin")))]
pub(crate) mod browser;
#[cfg(feature = "kotlin")]
pub(crate) mod callback;
#[cfg(feature = "kotlin")]
pub(crate) mod class_loader;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) mod desktop;
#[cfg(feature = "kotlin")]
pub(crate) mod discovery;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) mod inspector;
#[cfg(feature = "kotlin")]
pub(crate) mod jni_values;
#[cfg(feature = "kotlin")]
pub(crate) mod jvm;
#[cfg(feature = "kotlin")]
pub(crate) mod loader;
#[cfg(feature = "kotlin")]
pub(crate) mod metadata;
#[cfg(feature = "kotlin")]
pub(crate) mod option_metadata;
#[cfg(feature = "kotlin")]
pub(crate) mod patch_text;
#[cfg(feature = "kotlin")]
pub(crate) mod patch_translations;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) mod target_check;

#[cfg(all(feature = "browser", not(feature = "kotlin")))]
pub(crate) use browser::load_patches;
#[cfg(feature = "kotlin")]
pub(crate) use loader::load_patches;
