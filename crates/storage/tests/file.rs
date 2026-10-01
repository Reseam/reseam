// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{Read, Seek, SeekFrom, Write};

use reseam_storage::file::FileReader;

#[test]
fn shared_files_keep_independent_read_and_seek_positions() {
    let bytes: Vec<_> = (0..=255).cycle().take(16 * 1024).collect();
    let mut file = tempfile::tempfile().expect("fixture");
    file.write_all(&bytes).expect("fixture");
    let mut first = FileReader::new(file).expect("reader");
    let mut second = first.clone();
    let mut buffer = [0; 64];
    first.read_exact(&mut buffer).expect("first read");
    assert_eq!(buffer, bytes[..64]);
    second.read_exact(&mut buffer).expect("second read");
    assert_eq!(buffer, bytes[..64]);
    first.seek(SeekFrom::Start(4097)).expect("seek");
    second.read_exact(&mut buffer).expect("second cursor");
    assert_eq!(buffer, bytes[64..128]);
    assert!(first.seek(SeekFrom::Current(-4098)).is_err());
    first
        .read_exact(&mut buffer)
        .expect("failed seek preserves cursor");
    assert_eq!(buffer, bytes[4097..4161]);
}
