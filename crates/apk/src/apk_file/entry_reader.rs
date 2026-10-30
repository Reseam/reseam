// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, Cursor, Read};
use std::sync::Arc;

use reseam_storage::file::FileReader;

use crate::zip::entry_data::EntryData;

/// An entry's bytes as they were when it was opened, read without loading the
/// whole entry: original data decodes as it is read, and edits read from the
/// files that hold them. Later edits and rollbacks do not change what it reads.
pub struct EntryReader(Source);

enum Source {
    Archive(Box<EntryData>),
    File(FileReader),
    Memory(Cursor<Vec<u8>>),
}

impl EntryReader {
    pub(in crate::apk_file) fn archive(data: EntryData) -> Self {
        Self(Source::Archive(Box::new(data)))
    }

    pub(in crate::apk_file) fn file(file: impl Into<Arc<File>>) -> io::Result<Self> {
        Ok(Self(Source::File(FileReader::new(file)?)))
    }

    pub(in crate::apk_file) fn memory(bytes: Vec<u8>) -> Self {
        Self(Source::Memory(Cursor::new(bytes)))
    }
}

impl Read for EntryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Source::Archive(data) => data.read(buf),
            Source::File(file) => file.read(buf),
            Source::Memory(bytes) => bytes.read(buf),
        }
    }
}
