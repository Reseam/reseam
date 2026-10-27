// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Profile fields as [`super::cursor::Cursor`] reads them back.

use reseam_bytes::{write_u16, write_u32};
use std::io::Write;

use super::SECTION;
use crate::error::{Result, invalid};

pub(super) fn header(magic: [u8; 4], version: [u8; 4]) -> Vec<u8> {
    [magic, version].concat()
}

pub(super) fn write_ascending(out: &mut Vec<u8>, values: &[u32]) {
    let mut last = 0;
    for &value in values {
        write_u16(out, (value - last) as u16);
        last = value;
    }
}

pub(super) fn write_key(out: &mut Vec<u8>, key: &str) -> Result<()> {
    write_u16(out, u16_len(key.len())?);
    out.extend_from_slice(key.as_bytes());
    Ok(())
}

pub(super) fn write_deflated(out: &mut Vec<u8>, body: &[u8]) -> Result<()> {
    let deflated = deflate(body)?;
    write_u32(out, body.len() as u32);
    write_u32(out, deflated.len() as u32);
    out.extend_from_slice(&deflated);
    Ok(())
}

pub(super) fn deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub(super) fn u16_len(len: usize) -> Result<u16> {
    u16::try_from(len).map_err(|_| invalid(SECTION, format!("{len} exceeds 65535")))
}
