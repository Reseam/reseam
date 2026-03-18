use crate::error::{
    checksum_mismatch, invalid, invalid_magic, read_u16_le, read_u32_le, require_len,
    signature_mismatch, slice, truncated, unsupported, Result,
};
use crate::types::header::{DexHeader, DexVersion, ParseOptions};

pub fn read_header(buf: &[u8], opts: &ParseOptions) -> Result<DexHeader> {
    read_header_at(buf, 0, buf.len() as u32, opts)
}

pub fn read_header_at(
    buf: &[u8],
    header_off: usize,
    _container_len: u32,
    opts: &ParseOptions,
) -> Result<DexHeader> {
    require_len(buf, header_off, 112, "dex header")?;

    let mut magic = [0u8; 8];
    magic.copy_from_slice(slice(buf, header_off, 8, "dex header")?);

    let version = DexVersion::from_magic(&magic).ok_or_else(|| {
        if &buf[header_off..header_off + 4] == b"cdex" {
            unsupported("dex version", "Compact DEX (CDEX) is not supported")
        } else {
            invalid_magic(magic)
        }
    })?;

    let expected_header_size = version.header_size();
    require_len(buf, header_off, expected_header_size as usize, "dex header")?;

    let checksum = u32_at(buf, header_off + 0x08)?;
    let mut signature = [0u8; 20];
    signature.copy_from_slice(slice(buf, header_off + 0x0C, 20, "dex header")?);
    let file_size = u32_at(buf, header_off + 0x20)?;
    let header_size = u32_at(buf, header_off + 0x24)?;
    let endian_tag = u32_at(buf, header_off + 0x28)?;

    if header_size != expected_header_size {
        return Err(invalid(
            "dex header",
            format!(
                "invalid header size: expected {expected_header_size:#x}, got {header_size:#x}"
            ),
        ));
    }
    if endian_tag != 0x12345678 {
        return Err(invalid(
            "dex header",
            format!("invalid endian tag: expected 0x12345678, got {endian_tag:#010x}"),
        ));
    }

    let (container_size, header_offset) = if version.is_container_format() {
        let cs = u32_at(buf, header_off + 0x70)?;
        let ho = u32_at(buf, header_off + 0x74)?;
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

    let logical_end = header_off as u32 + file_size;
    if logical_end as usize > buf.len() {
        return Err(truncated(
            "dex file",
            header_off,
            file_size as usize,
            buf.len() - header_off,
        ));
    }

    if !opts.skip_checksum {
        let computed = adler::adler32(&buf[header_off + 12..logical_end as usize])
            .map_err(|err| invalid("dex header", format!("failed to compute checksum: {err}")))?;
        if computed != checksum {
            return Err(checksum_mismatch(checksum, computed));
        }
    }

    if !opts.skip_signature {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(&buf[header_off + 32..logical_end as usize]);
        let computed: [u8; 20] = hasher.finalize().into();
        if computed != signature {
            return Err(signature_mismatch());
        }
    }

    Ok(DexHeader {
        version,
        checksum,
        signature,
        file_size,
        link_size: u32_at(buf, header_off + 0x2C)?,
        link_off: u32_at(buf, header_off + 0x30)?,
        map_off: u32_at(buf, header_off + 0x34)?,
        string_ids_size: u32_at(buf, header_off + 0x38)?,
        string_ids_off: u32_at(buf, header_off + 0x3C)?,
        type_ids_size: u32_at(buf, header_off + 0x40)?,
        type_ids_off: u32_at(buf, header_off + 0x44)?,
        proto_ids_size: u32_at(buf, header_off + 0x48)?,
        proto_ids_off: u32_at(buf, header_off + 0x4C)?,
        field_ids_size: u32_at(buf, header_off + 0x50)?,
        field_ids_off: u32_at(buf, header_off + 0x54)?,
        method_ids_size: u32_at(buf, header_off + 0x58)?,
        method_ids_off: u32_at(buf, header_off + 0x5C)?,
        class_defs_size: u32_at(buf, header_off + 0x60)?,
        class_defs_off: u32_at(buf, header_off + 0x64)?,
        data_size: u32_at(buf, header_off + 0x68)?,
        data_off: u32_at(buf, header_off + 0x6C)?,
        container_size,
        header_offset,
    })
}

pub(crate) fn u16_at(buf: &[u8], off: usize) -> Result<u16> {
    read_u16_le(buf, off, "dex data")
}

pub(crate) fn u32_at(buf: &[u8], off: usize) -> Result<u32> {
    read_u32_le(buf, off, "dex data")
}
