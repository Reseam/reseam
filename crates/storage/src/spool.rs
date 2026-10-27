// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufWriter, Write};

use crate::MappedFile;

const BUFFER: usize = 1 << 20;

/// Writes what `write` produces to a new unlinked temporary file, through a
/// buffer large enough that small writes do not each cost a system call.
pub fn spool<E: From<io::Error>>(
    write: impl FnOnce(&mut dyn Write) -> Result<(), E>,
) -> Result<File, E> {
    let mut file = crate::temporary_file()?;
    let mut out = BufWriter::with_capacity(BUFFER, &mut file);
    write(&mut out)?;
    out.flush()?;
    drop(out);
    Ok(file)
}

/// [`spool`], mapped for reading.
pub fn spool_mapped<E: From<io::Error>>(
    write: impl FnOnce(&mut dyn Write) -> Result<(), E>,
) -> Result<MappedFile, E> {
    let file = spool(write)?;
    // SAFETY: the file is unlinked and this function holds its only handle,
    // so nothing can change it while the mapping lives.
    Ok(unsafe { crate::map_file(&file)? })
}

/// Copies the whole of `file` to `output` without moving a cursor that other
/// users of the file share.
pub fn copy_file(file: &File, output: &mut impl Write) -> io::Result<()> {
    #[cfg(not(target_os = "wasi"))]
    let mut file = io::BufReader::new(crate::file::FileReader::new(file.try_clone()?)?);
    #[cfg(target_os = "wasi")]
    let mut file = {
        // WASI has one engine thread and cannot clone file descriptors.
        let mut source = file;
        io::Seek::seek(&mut source, io::SeekFrom::Start(0))?;
        io::BufReader::new(source)
    };
    io::copy(&mut file, output)?;
    Ok(())
}
