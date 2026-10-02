// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(not(target_os = "wasi"))]
pub use std::fs::canonicalize;

/// Resolves a path within a WASI host's root-mounted, symlink-free filesystem.
/// Browser hosts expose only regular files and directories under `/`.
#[cfg(target_os = "wasi")]
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> std::io::Result<std::path::PathBuf> {
    use std::path::{Component, PathBuf};
    let mut resolved = PathBuf::from("/");
    for component in path.as_ref().components() {
        match component {
            Component::RootDir => resolved = PathBuf::from("/"),
            Component::CurDir => {}
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir if resolved.pop() => {}
            Component::ParentDir | Component::Prefix(_) => {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
        }
    }
    std::fs::metadata(&resolved)?;
    Ok(resolved)
}
