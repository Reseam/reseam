// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, Seek, Write};
use std::path::Path;

/// Places the written `file` at `path`: a link to it where the system can
/// name an open file, a copy elsewhere.
pub(crate) fn stage_file(file: &File, path: &Path) -> io::Result<()> {
    match link_anonymous(file, path) {
        Ok(()) => return Ok(()),
        Err(error) if matches!(error.kind(), io::ErrorKind::AlreadyExists) => {
            return Err(error);
        }
        Err(_) => {}
    }
    let mut reader = io::BufReader::new(file);
    reader.seek(io::SeekFrom::Start(0))?;
    let mut writer = io::BufWriter::new(File::options().write(true).create_new(true).open(path)?);
    io::copy(&mut reader, &mut writer)?;
    writer.flush()
}

#[cfg(target_os = "linux")]
fn link_anonymous(file: &File, path: &Path) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let target = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    // SAFETY: both paths are valid C strings; linkat only creates a directory entry.
    let result = unsafe {
        libc::linkat(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::AT_SYMLINK_FOLLOW,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "linux"))]
fn link_anonymous(_file: &File, _path: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
