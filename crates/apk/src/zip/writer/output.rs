// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{self, BufWriter, Seek, SeekFrom, Write};

use crate::error::Result;

/// The output file and how much of it is written.
pub(super) struct Output<W: Write + Seek> {
    pub(super) file: BufWriter<W>,
    pub(super) position: u64,
}

impl<W: Write + Seek> Write for Output<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buf)?;
        self.position += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl<W: Write + Seek> Output<W> {
    /// Overwrites bytes written earlier at `offset`.
    pub(super) fn patch(&mut self, offset: u64, bytes: &[u8]) -> Result<()> {
        self.file.flush()?;
        let file = self.file.get_mut();
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(bytes)?;
        file.seek(SeekFrom::Start(self.position))?;
        Ok(())
    }
}
