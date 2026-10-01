// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::{fs, io};

const PREFIX: &str = "reseam-";

#[derive(Debug)]
pub struct ScratchDir {
    dir: tempfile::TempDir,
}

impl ScratchDir {
    /// Creates `<TMPDIR>/reseam-<pid>-<label>-<random>` after sweeping directories
    /// left by processes that no longer exist.
    pub fn new(label: &str) -> io::Result<Self> {
        let root = std::env::temp_dir();
        sweep_stale(&root);
        let dir = tempfile::Builder::new()
            .prefix(&format!("{PREFIX}{}-{label}-", std::process::id()))
            .tempdir_in(root)?;
        Ok(Self { dir })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
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
            // Stale-file cleanup is best effort; it must not prevent creating a new run.
            drop(fs::remove_dir_all(entry.path()));
        }
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 checks for existence and permission without sending anything.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    rc == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: the handle is only queried for its exit code and closed again.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut exit_code = 0;
        let alive =
            GetExitCodeProcess(process, &mut exit_code) != 0 && exit_code == STILL_ACTIVE as u32;
        CloseHandle(process);
        alive
    }
}
