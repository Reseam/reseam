// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::store::{Caching, Storage};

/// A folder a web server serves the releases from.
pub(super) struct Directory {
    root: PathBuf,
}

impl Directory {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl fmt::Display for Directory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display().fmt(f)
    }
}

impl Storage for Directory {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match std::fs::read_to_string(self.root.join(key)) {
            Ok(json) => Ok(Some(json)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("failed to read the published index"),
        }
    }

    /// Copies into a new file, so the published copy gets the default permissions
    /// a web server can read.
    fn put(&self, key: &str, path: &Path, _caching: Caching) -> Result<()> {
        let target = self.root.join(key);
        let context = || format!("failed to copy {} to {}", path.display(), target.display());
        if let Some(folder) = target.parent() {
            std::fs::create_dir_all(folder)
                .with_context(|| format!("failed to create {}", folder.display()))?;
        }
        let mut source = std::fs::File::open(path).with_context(context)?;
        let mut copy = std::fs::File::create(&target).with_context(context)?;
        std::io::copy(&mut source, &mut copy).with_context(context)?;
        Ok(())
    }

    fn folders(&self) -> Result<Vec<String>> {
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Ok(name) = entry.file_name().into_string()
            {
                folders.push(name);
            }
        }
        Ok(folders)
    }

    fn delete_folder(&self, name: &str) -> Result<()> {
        Ok(std::fs::remove_dir_all(self.root.join(name))?)
    }

    fn public_path(&self, key: &str) -> String {
        key.to_owned()
    }
}
