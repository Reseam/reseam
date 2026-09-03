// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scratch directories for files that must have a path while a run is
//! alive (extracted patch jars, ART's dex cache). Each is named after the
//! owning process, removed on drop, and swept on the next run if its owner
//! died first, so a killed run leaves nothing behind for long.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{fs, io};

const PREFIX: &str = "reseam-";

static SEQUENCE: AtomicU32 = AtomicU32::new(0);

pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates `<TMPDIR>/reseam-<pid>-<label>-<n>` after sweeping directories
    /// left by processes that no longer exist.
    pub fn new(label: &str) -> io::Result<Self> {
        let root = std::env::temp_dir();
        sweep_stale(&root);
        let n = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!("{PREFIX}{}-{label}-{n}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn sweep_stale(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let own = std::process::id();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix(PREFIX))
            .and_then(|rest| rest.split('-').next())
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != own && !process_alive(pid) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 checks for existence and permission without sending anything.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    rc == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_owner_directories_are_swept() {
        let root = std::env::temp_dir();
        let stale = root.join(format!("{PREFIX}4000000000-test-0"));
        fs::create_dir_all(stale.join("inner")).unwrap();
        let live = ScratchDir::new("test").unwrap();
        assert!(live.path().is_dir());
        assert!(!stale.exists());
        let kept = live.path().to_path_buf();
        drop(live);
        assert!(!kept.exists());
    }
}
