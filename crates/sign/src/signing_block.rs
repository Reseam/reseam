use crate::error::{invalid, malformed, Result};

const APK_SIG_BLOCK_MAGIC: &[u8; 16] = b"APK Sig Block 42";
const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const PAIR_OVERHEAD: usize = 12;
const BLOCK_OVERHEAD: usize = 32;
const BLOCK_ID_PADDING: u32 = 0x4272_6577;

pub const BLOCK_ID_V2: u32 = 0x7109871a;
pub const BLOCK_ID_V3: u32 = 0xf05368c0;

pub struct ApkSections<'a> {
    pub contents: &'a [u8],
    pub central_dir: &'a [u8],
    pub eocd: &'a [u8],
    pub cd_offset: u32,
}

/// Returns (eocd_offset, cd_offset, cd_size).
pub fn find_eocd(data: &[u8]) -> Result<(usize, u32, u32)> {
    let min_eocd_size = 22;
    if data.len() < min_eocd_size {
        return Err(invalid("apk", "file too small for ZIP"));
    }

    let search_start = data.len().saturating_sub(min_eocd_size + 65535);
    for i in (search_start..=data.len() - min_eocd_size).rev() {
        if &data[i..i + 4] == EOCD_SIGNATURE {
            let comment_len = read_u16_le(data, i + 20)
                .ok_or_else(|| malformed("zip eocd", i + 20, "truncated EOCD record"))?
                as usize;
            if i + min_eocd_size + comment_len != data.len() {
                continue;
            }

            let cd_size = read_u32_le(data, i + 12).ok_or_else(|| {
                malformed("zip eocd", i + 12, "truncated EOCD central directory size")
            })?;
            let cd_offset = read_u32_le(data, i + 16).ok_or_else(|| {
                malformed(
                    "zip eocd",
                    i + 16,
                    "truncated EOCD central directory offset",
                )
            })?;
            return Ok((i, cd_offset, cd_size));
        }
    }

    Err(invalid("apk", "EOCD not found"))
}

/// Strips any existing signing block and returns the three ZIP sections.
pub fn split_apk(data: &[u8]) -> Result<ApkSections<'_>> {
    let (eocd_offset, cd_offset, _cd_size) = find_eocd(data)?;
    let cd_offset = cd_offset as usize;

    if cd_offset > eocd_offset || cd_offset > data.len() {
        return Err(invalid("apk", "invalid central directory offset"));
    }

    let contents_end = find_signing_block_start(data, cd_offset).unwrap_or(cd_offset);

    Ok(ApkSections {
        contents: &data[..contents_end],
        central_dir: &data[cd_offset..eocd_offset],
        eocd: &data[eocd_offset..],
        cd_offset: cd_offset as u32,
    })
}

fn find_signing_block_start(data: &[u8], cd_offset: usize) -> Option<usize> {
    if cd_offset < 24 || cd_offset > data.len() {
        return None;
    }

    let magic_start = cd_offset - 16;
    if data.get(magic_start..cd_offset)? != APK_SIG_BLOCK_MAGIC {
        return None;
    }

    let block_size = read_u64_le(data, magic_start.checked_sub(8)?)? as usize;

    // Total block = 8 (leading size) + block_size
    let block_start = cd_offset.checked_sub(8 + block_size)?;
    if block_start + 8 > data.len() {
        return None;
    }

    let leading_size = read_u64_le(data, block_start)? as usize;

    if leading_size != block_size {
        return None;
    }

    Some(block_start)
}

pub fn build_signing_block(pairs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let pairs_size: usize = pairs.iter().map(|(_, v)| 8 + 4 + v.len()).sum();
    let block_size = pairs_size + 8 + 16;
    let mut block = Vec::with_capacity(8 + block_size);

    block.extend_from_slice(&(block_size as u64).to_le_bytes());

    for (id, value) in pairs {
        block.extend_from_slice(&((4 + value.len()) as u64).to_le_bytes());
        block.extend_from_slice(&id.to_le_bytes());
        block.extend_from_slice(value);
    }

    block.extend_from_slice(&(block_size as u64).to_le_bytes());
    block.extend_from_slice(APK_SIG_BLOCK_MAGIC);

    block
}

pub(crate) fn signing_block_len(value_lengths: &[usize]) -> usize {
    BLOCK_OVERHEAD
        + value_lengths
            .iter()
            .map(|len| PAIR_OVERHEAD + len)
            .sum::<usize>()
}

pub(crate) fn build_signing_block_with_padding(
    pairs: &[(u32, Vec<u8>)],
    target_len: usize,
) -> Result<Vec<u8>> {
    let current_len = signing_block_len(
        &pairs
            .iter()
            .map(|(_, value)| value.len())
            .collect::<Vec<_>>(),
    );

    if current_len == target_len {
        return Ok(build_signing_block(pairs));
    }

    let min_padded_len = current_len.checked_add(PAIR_OVERHEAD).ok_or_else(|| {
        malformed(
            "signing block",
            current_len,
            "length overflowed while padding",
        )
    })?;
    if min_padded_len > target_len {
        return Err(invalid(
            "signing block",
            format!("signing block exceeded target length ({current_len} > {target_len})"),
        ));
    }

    let padding_len = target_len - min_padded_len;
    let mut padded_pairs = pairs.to_vec();
    padded_pairs.push((BLOCK_ID_PADDING, vec![0; padding_len]));

    let block = build_signing_block(&padded_pairs);
    debug_assert_eq!(block.len(), target_len);
    Ok(block)
}

/// Inserts signing block between contents and central directory,
/// patches EOCD to point to the new CD offset.
pub fn reassemble_apk(
    contents: &[u8],
    signing_block: &[u8],
    central_dir: &[u8],
    eocd: &[u8],
) -> Result<Vec<u8>> {
    let new_cd_offset = contents.len() + signing_block.len();
    let mut output =
        Vec::with_capacity(contents.len() + signing_block.len() + central_dir.len() + eocd.len());

    output.extend_from_slice(contents);
    output.extend_from_slice(signing_block);
    output.extend_from_slice(central_dir);

    let mut patched_eocd = eocd.to_vec();
    if patched_eocd.len() < 22 {
        return Err(malformed("eocd", 0, "EOCD record too short (< 22 bytes)"));
    }
    patched_eocd[16..20].copy_from_slice(&(new_cd_offset as u32).to_le_bytes());
    output.extend_from_slice(&patched_eocd);

    Ok(output)
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64_le(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
