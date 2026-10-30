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
