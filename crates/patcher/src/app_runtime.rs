// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The engine's code for patched apps: the settings store and screen, and the branded components
//! extensions build on. Patches link it like an extension; bundles may not define its classes.

#[cfg(feature = "bridge")]
include!(concat!(env!("OUT_DIR"), "/app_runtime.rs"));

/// The descriptor prefix of every app runtime class.
pub(crate) const PACKAGE: &str = "Lapp/reseam/runtime/";

/// The app runtime's JavaScript module compiled to Hermes bytecode `version`.
#[cfg(feature = "bridge")]
pub(crate) fn hermes(version: u32) -> Option<&'static [u8]> {
    HERMES
        .iter()
        .find(|(supported, _)| *supported == version)
        .map(|(_, module)| *module)
}
