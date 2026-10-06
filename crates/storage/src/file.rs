// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

/// Reads and writes at an explicit offset, so one descriptor can be used from
/// several threads at once and a writer never has to track where the cursor
/// is. Callers must not rely on the cursor afterwards: Windows moves it.
pub trait FileExt {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize>;

    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read_at(buf, offset) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.write_at(buf, offset) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    buf = &buf[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
impl FileExt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        std::os::unix::fs::FileExt::read_at(self, buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        std::os::unix::fs::FileExt::write_at(self, buf, offset)
    }
}

#[cfg(windows)]
impl FileExt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        std::os::windows::fs::FileExt::seek_read(self, buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        std::os::windows::fs::FileExt::seek_write(self, buf, offset)
    }
}

#[cfg(target_os = "wasi")]
impl FileExt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let mut file = self;
        file.seek(SeekFrom::Start(offset))?;
        file.read(buf)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        let mut file = self;
        file.seek(SeekFrom::Start(offset))?;
        io::Write::write(&mut file, buf)
    }
}

/// A positional file reader: clones share the descriptor but keep their own
/// offset.
#[derive(Clone)]
pub struct FileReader {
    file: Arc<File>,
    pos: u64,
    len: u64,
}

impl FileReader {
    /// Creates an independent positional cursor over an open file. Clones
    /// share the file descriptor and retain their own seek position. End-relative
    /// seeks use the length observed here; the source must remain unchanged.
    pub fn new(file: File) -> io::Result<Self> {
        let len = file.metadata()?.len();
        Ok(Self {
            file: Arc::new(file),
            pos: 0,
            len,
        })
    }

    pub fn file(&self) -> &File {
        &self.file
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read_at(buf, self.pos)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for FileReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (base, delta) = match pos {
            SeekFrom::Start(offset) => (offset, 0),
            SeekFrom::End(delta) => (self.len, delta),
            SeekFrom::Current(delta) => (self.pos, delta),
        };
        self.pos = base
            .checked_add_signed(delta)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}

/// The host-mounted temporary directory. WASI does not implement `env::temp_dir`.
pub fn temp_root() -> std::path::PathBuf {
    #[cfg(target_os = "wasi")]
    {
        std::path::PathBuf::from("/tmp")
    }
    #[cfg(not(target_os = "wasi"))]
    {
        std::env::temp_dir()
    }
}

/// Creates an unlinked temporary file under the host's scratch directory.
pub fn temporary_file() -> io::Result<File> {
    #[cfg(target_os = "wasi")]
    {
        tempfile::tempfile_in(temp_root())
    }
    #[cfg(not(target_os = "wasi"))]
    {
        tempfile::tempfile()
    }
}
