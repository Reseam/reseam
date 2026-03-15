use crate::error::{DexError, Result};
use crate::model::header::{DexHeader, DexVersion, ParseOptions};

pub fn read_header(buf: &[u8], opts: &ParseOptions) -> Result<DexHeader> {
    if buf.len() < 112 {
        return Err(DexError::FileTruncated {
            expected: 112,
            actual: buf.len(),
        });
    }

    let mut magic = [0u8; 8];
    magic.copy_from_slice(&buf[0..8]);

    let version = DexVersion::from_magic(&magic).ok_or_else(|| {
        if &buf[0..4] == b"cdex" {
            DexError::UnsupportedVersion {
                version: "Compact DEX (CDEX) is not supported".into(),
            }
        } else {
            DexError::InvalidMagic { found: magic }
        }
    })?;

    let checksum = u32_at(buf, 0x08);
    let mut signature = [0u8; 20];
    signature.copy_from_slice(&buf[0x0C..0x20]);
    let file_size = u32_at(buf, 0x20);
    let header_size = u32_at(buf, 0x24);
    let endian_tag = u32_at(buf, 0x28);

    if header_size != 0x70 {
        return Err(DexError::InvalidHeaderSize { size: header_size });
    }
    if endian_tag != 0x12345678 {
        return Err(DexError::InvalidEndianTag { tag: endian_tag });
    }
    if file_size as usize != buf.len() {
        return Err(DexError::FileTruncated {
            expected: file_size as usize,
            actual: buf.len(),
        });
    }

    if !opts.skip_checksum {
        let computed = adler::adler32(&buf[12..file_size as usize]).unwrap();
        if computed != checksum {
            return Err(DexError::ChecksumMismatch {
                expected: checksum,
                computed,
            });
        }
    }

    if !opts.skip_signature {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(&buf[32..file_size as usize]);
        let computed: [u8; 20] = hasher.finalize().into();
        if computed != signature {
            return Err(DexError::SignatureMismatch);
        }
    }

    Ok(DexHeader {
        version,
        checksum,
        signature,
        file_size,
        link_size: u32_at(buf, 0x2C),
        link_off: u32_at(buf, 0x30),
        map_off: u32_at(buf, 0x34),
        string_ids_size: u32_at(buf, 0x38),
        string_ids_off: u32_at(buf, 0x3C),
        type_ids_size: u32_at(buf, 0x40),
        type_ids_off: u32_at(buf, 0x44),
        proto_ids_size: u32_at(buf, 0x48),
        proto_ids_off: u32_at(buf, 0x4C),
        field_ids_size: u32_at(buf, 0x50),
        field_ids_off: u32_at(buf, 0x54),
        method_ids_size: u32_at(buf, 0x58),
        method_ids_off: u32_at(buf, 0x5C),
        class_defs_size: u32_at(buf, 0x60),
        class_defs_off: u32_at(buf, 0x64),
        data_size: u32_at(buf, 0x68),
        data_off: u32_at(buf, 0x6C),
    })
}

pub(crate) fn u16_at(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(
        buf.get(off..off + 2)
            .expect("u16_at: offset out of bounds")
            .try_into()
            .unwrap(),
    )
}

pub(crate) fn u32_at(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(
        buf.get(off..off + 4)
            .expect("u32_at: offset out of bounds")
            .try_into()
            .unwrap(),
    )
}
