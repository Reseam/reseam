// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{
    Result, checksum_mismatch, invalid, invalid_magic, require_len, signature_mismatch, slice,
    truncated, unsupported,
};
use crate::read::u32_at;
use crate::types::header::{DexHeader, DexVersion, ParseOptions};

pub fn read_header_at(buf: &[u8], header_off: usize, opts: ParseOptions) -> Result<DexHeader> {
    require_len(buf, header_off, 112, "dex header")?;

    let mut magic = [0u8; 8];
    magic.copy_from_slice(slice(buf, header_off, 8, "dex header")?);

    let version = DexVersion::from_magic(magic).ok_or_else(|| {
        if &buf[header_off..header_off + 4] == b"cdex" {
            unsupported("dex version", "Compact DEX (CDEX) is not supported")
        } else {
            invalid_magic(magic)
        }
    })?;

    let expected_header_size = version.header_size();
    require_len(buf, header_off, expected_header_size as usize, "dex header")?;

    let checksum = u32_at(buf, header_off + 0x08);
    let mut signature = [0u8; 20];
    signature.copy_from_slice(slice(buf, header_off + 0x0C, 20, "dex header")?);
    let file_size = u32_at(buf, header_off + 0x20);
    let header_size = u32_at(buf, header_off + 0x24);
    let endian_tag = u32_at(buf, header_off + 0x28);

    if header_size != expected_header_size {
        return Err(invalid(
            "dex header",
            format!(
                "invalid header size: expected {expected_header_size:#x}, got {header_size:#x}"
            ),
        ));
    }
    if file_size < header_size {
        return Err(invalid(
            "dex header",
            format!("file size {file_size:#x} is smaller than header size {header_size:#x}"),
        ));
    }
    if endian_tag != 0x1234_5678 {
        return Err(invalid(
            "dex header",
            format!("invalid endian tag: expected 0x1234_5678, got {endian_tag:#010x}"),
        ));
    }

    let (container_size, header_offset) = if version.is_container_format() {
        let cs = u32_at(buf, header_off + 0x70);
        let ho = u32_at(buf, header_off + 0x74);
        if ho as usize != header_off {
            return Err(invalid(
                "dex header",
                format!(
                    "header_offset mismatch: field says {ho:#x}, actual position is {header_off:#x}"
                ),
            ));
        }
        (cs, ho)
    } else {
        (file_size, header_off as u32)
    };

    let logical_end = header_off
        .checked_add(file_size as usize)
        .filter(|&end| end <= buf.len())
        .ok_or_else(|| {
            truncated(
                "dex file",
                header_off,
                file_size as usize,
                buf.len() - header_off,
            )
        })?;

    if version.is_container_format()
        && (container_size as usize > buf.len() || logical_end > container_size as usize)
    {
        return Err(invalid(
            "dex container",
            "logical member exceeds declared container span",
        ));
    }
    let link_size = u32_at(buf, header_off + 0x2c);
    let link_off = u32_at(buf, header_off + 0x30);
    if link_size != 0 {
        require_len(
            &buf[..container_size as usize],
            link_off as usize,
            link_size as usize,
            "link data",
        )?;
    }
    verify_integrity(buf, header_off, logical_end, checksum, &signature, opts)?;

    Ok(header_fields(
        buf,
        header_off,
        version,
        checksum,
        signature,
        container_size,
        header_offset,
    ))
}

fn header_fields(
    buf: &[u8],
    header_off: usize,
    version: DexVersion,
    checksum: u32,
    signature: [u8; 20],
    container_size: u32,
    header_offset: u32,
) -> DexHeader {
    DexHeader {
        version,
        checksum,
        signature,
        file_size: u32_at(buf, header_off + 0x20),
        link_size: u32_at(buf, header_off + 0x2c),
        link_off: u32_at(buf, header_off + 0x30),
        map_off: u32_at(buf, header_off + 0x34),
        string_ids_size: u32_at(buf, header_off + 0x38),
        string_ids_off: u32_at(buf, header_off + 0x3C),
        type_ids_size: u32_at(buf, header_off + 0x40),
        type_ids_off: u32_at(buf, header_off + 0x44),
        proto_ids_size: u32_at(buf, header_off + 0x48),
        proto_ids_off: u32_at(buf, header_off + 0x4C),
        field_ids_size: u32_at(buf, header_off + 0x50),
        field_ids_off: u32_at(buf, header_off + 0x54),
        method_ids_size: u32_at(buf, header_off + 0x58),
        method_ids_off: u32_at(buf, header_off + 0x5C),
        class_defs_size: u32_at(buf, header_off + 0x60),
        class_defs_off: u32_at(buf, header_off + 0x64),
        data_size: u32_at(buf, header_off + 0x68),
        data_off: u32_at(buf, header_off + 0x6C),
        container_size,
        header_offset,
    }
}

fn verify_integrity(
    buf: &[u8],
    header_off: usize,
    logical_end: usize,
    checksum: u32,
    signature: &[u8; 20],
    opts: ParseOptions,
) -> Result<()> {
    if !(opts.checksum == crate::types::header::Verification::Skip) {
        let computed = zlib_rs::adler32::adler32(1, &buf[header_off + 12..logical_end]);
        if computed != checksum {
            return Err(checksum_mismatch(checksum, computed));
        }
    }

    if !(opts.signature == crate::types::header::Verification::Skip) {
        let computed = ring::digest::digest(
            &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
            &buf[header_off + 32..logical_end],
        );
        if computed.as_ref() != signature.as_slice() {
            return Err(signature_mismatch());
        }
    }

    Ok(())
}
