// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Read, Result, Write};

use sha2::{Digest, Sha256};

/// Streams `reader` into `writer` and returns the SHA-256 of what it copied.
pub fn copy_hashed(reader: &mut impl Read, writer: &mut impl Write) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let size = reader.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        writer.write_all(&buffer[..size])?;
        hash.update(&buffer[..size]);
    }
    Ok(hash.finalize().into())
}
