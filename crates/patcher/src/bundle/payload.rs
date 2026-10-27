// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use crate::error::{BundleError, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PayloadKind {
    Jar,
    Dex,
    Resource,
}

/// Windows device names, reserved in every directory with any extension.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// What the payload entry `name` holds: patch jars and extension DEX at the
/// root, anything under `resources/`.
pub(super) fn payload_kind(name: &str) -> Result<PayloadKind> {
    if !name.split('/').all(is_portable) {
        return Err(BundleError::UnsafePath(name.to_owned()).into());
    }
    let extension = Path::new(name).extension();
    if name.starts_with("resources/") {
        Ok(PayloadKind::Resource)
    } else if !name.contains('/') && extension.is_some_and(|extension| extension == "jar") {
        Ok(PayloadKind::Jar)
    } else if !name.contains('/') && extension.is_some_and(|extension| extension == "dex") {
        Ok(PayloadKind::Dex)
    } else {
        Err(BundleError::UnsupportedPayload(name.to_owned()).into())
    }
}

/// Whether a path part means the same file on every host. Windows interprets
/// backslashes, drive prefixes and device names even in archives made on Unix.
fn is_portable(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
    !(part.is_empty()
        || part == "."
        || part == ".."
        || part.contains(['\\', ':', '\0'])
        || part.ends_with(['.', ' '])
        || part.chars().any(char::is_control)
        || RESERVED_NAMES.contains(&stem.as_str()))
}
