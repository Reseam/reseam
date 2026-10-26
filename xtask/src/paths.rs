// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate lives one level below the workspace root")
        .to_path_buf()
}

pub fn target_dir() -> PathBuf {
    let root = workspace_root();
    env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |path| root.join(path))
}
