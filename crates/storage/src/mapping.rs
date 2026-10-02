// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io;

#[cfg(not(target_os = "wasi"))]
pub type MappedFile = memmap2::Mmap;
#[cfg(target_os = "wasi")]
pub type MappedFile = Vec<u8>;

/// Reads an immutable file range. Native hosts map it; WASI hosts copy only
/// the requested range because WASI has no memory mapping facility.
///
/// # Safety
/// The file must remain unchanged for the lifetime of the returned mapping.
pub unsafe fn map_range(file: &File, offset: u64, len: usize) -> io::Result<MappedFile> {
    #[cfg(not(target_os = "wasi"))]
    {
        // SAFETY: the caller guarantees an immutable source for the mapping's lifetime.
        unsafe {
            memmap2::MmapOptions::new()
                .offset(offset)
                .len(len)
                .map(file)
        }
    }
    #[cfg(target_os = "wasi")]
    {
        use crate::file::FileExt;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(len).map_err(io::Error::other)?;
        bytes.resize(len, 0);
        file.read_exact_at(&mut bytes, offset)?;
        Ok(bytes)
    }
}

/// Reads an entire immutable file, with the same lifetime contract as [`map_range`].
///
/// # Safety
/// The file must remain unchanged for the lifetime of the returned mapping.
pub unsafe fn map_file(file: &File) -> io::Result<MappedFile> {
    let len = usize::try_from(file.metadata()?.len()).map_err(io::Error::other)?;
    // SAFETY: the caller guarantees the file remains immutable.
    unsafe { map_range(file, 0, len) }
}
