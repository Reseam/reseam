use crate::error::{
    checksum_mismatch, invalid, invalid_magic, read_u16_le, read_u32_le, require_len,
    signature_mismatch, slice, truncated, unsupported, Result,
};
use crate::model::header::{DexHeader, DexVersion, ParseOptions};

pub fn read_header(buf: &[u8], opts: &ParseOptions) -> Result<DexHeader> {
    require_len(buf, 0, 112, "dex header")?;

    let mut magic = [0u8; 8];
    magic.copy_from_slice(slice(buf, 0, 8, "dex header")?);

    let version = DexVersion::from_magic(&magic).ok_or_else(|| {
        if &buf[0..4] == b"cdex" {
            unsupported("dex version", "Compact DEX (CDEX) is not supported")
        } else {
            invalid_magic(magic)
        }
    })?;

    let checksum = u32_at(buf, 0x08)?;
    let mut signature = [0u8; 20];
    signature.copy_from_slice(slice(buf, 0x0C, 20, "dex header")?);
    let file_size = u32_at(buf, 0x20)?;
    let header_size = u32_at(buf, 0x24)?;
    let endian_tag = u32_at(buf, 0x28)?;

    if header_size != 0x70 {
        return Err(invalid(
            "dex header",
            format!("invalid header size: expected 0x70, got {header_size:#x}"),
        ));
    }
    if endian_tag != 0x12345678 {
        return Err(invalid(
            "dex header",
            format!("invalid endian tag: expected 0x12345678, got {endian_tag:#010x}"),
        ));
    }
    if file_size as usize != buf.len() {
        return Err(truncated("dex file", 0, file_size as usize, buf.len()));
    }

    if !opts.skip_checksum {
        let computed = adler::adler32(&buf[12..file_size as usize])
            .map_err(|err| invalid("dex header", format!("failed to compute checksum: {err}")))?;
        if computed != checksum {
            return Err(checksum_mismatch(checksum, computed));
        }
    }

    if !opts.skip_signature {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(&buf[32..file_size as usize]);
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
        link_size: u32_at(buf, 0x2C)?,
        link_off: u32_at(buf, 0x30)?,
        map_off: u32_at(buf, 0x34)?,
        string_ids_size: u32_at(buf, 0x38)?,
        string_ids_off: u32_at(buf, 0x3C)?,
        type_ids_size: u32_at(buf, 0x40)?,
        type_ids_off: u32_at(buf, 0x44)?,
        proto_ids_size: u32_at(buf, 0x48)?,
        proto_ids_off: u32_at(buf, 0x4C)?,
        field_ids_size: u32_at(buf, 0x50)?,
        field_ids_off: u32_at(buf, 0x54)?,
        method_ids_size: u32_at(buf, 0x58)?,
        method_ids_off: u32_at(buf, 0x5C)?,
        class_defs_size: u32_at(buf, 0x60)?,
        class_defs_off: u32_at(buf, 0x64)?,
        data_size: u32_at(buf, 0x68)?,
        data_off: u32_at(buf, 0x6C)?,
    })
}

pub(crate) fn u16_at(buf: &[u8], off: usize) -> Result<u16> {
    read_u16_le(buf, off, "dex data")
}

pub(crate) fn u32_at(buf: &[u8], off: usize) -> Result<u32> {
    read_u32_le(buf, off, "dex data")
}
