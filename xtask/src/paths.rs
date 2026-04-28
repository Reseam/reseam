// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live one level below the workspace root")
        .to_path_buf()
}

pub fn patcher_crate() -> PathBuf {
    workspace_root().join("crates/patcher")
}

pub fn patch_api() -> PathBuf {
    workspace_root().join("patch-api")
}

pub fn sdk() -> PathBuf {
    workspace_root().join("sdk")
}

pub fn target_debug() -> PathBuf {
    workspace_root().join("target/debug")
}
