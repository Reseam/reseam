use crate::error::{Result, SignError};

/// Magic bytes at the end of the APK Signing Block.
const APK_SIG_BLOCK_MAGIC: &[u8; 16] = b"APK Sig Block 42";

/// Block ID for APK Signature Scheme v2.
pub const BLOCK_ID_V2: u32 = 0x7109871a;

/// Block ID for APK Signature Scheme v3.
pub const BLOCK_ID_V3: u32 = 0xf05368c0;

/// Parsed APK structure with the three ZIP sections separated out.
pub struct ApkSections<'a> {
    /// Section 1: everything before the signing block or central directory.
    pub contents: &'a [u8],
    /// Section 3: ZIP central directory.
    pub central_dir: &'a [u8],
    /// Section 4: ZIP end of central directory record.
    pub eocd: &'a [u8],
    /// Offset where the central directory starts in the original file.
    pub cd_offset: u32,
}

/// Find the End of Central Directory record in a ZIP file.
/// Returns (eocd_offset, cd_offset, cd_size).
pub fn find_eocd(data: &[u8]) -> Result<(usize, u32, u32)> {
    // EOCD signature: 0x06054b50
    // Search backwards from the end (EOCD can have a comment, up to 65535 bytes)
    let min_eocd_size = 22;
    if data.len() < min_eocd_size {
        return Err(SignError::InvalidApk {
            reason: "file too small for ZIP".into(),
        });
    }

    let search_start = data.len().saturating_sub(min_eocd_size + 65535);
    for i in (search_start..=data.len() - min_eocd_size).rev() {
        if data[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            let cd_size = u32::from_le_bytes([data[i + 12], data[i + 13], data[i + 14], data[i + 15]]);
            let cd_offset = u32::from_le_bytes([data[i + 16], data[i + 17], data[i + 18], data[i + 19]]);
            return Ok((i, cd_offset, cd_size));
        }
    }

    Err(SignError::InvalidApk {
        reason: "EOCD not found".into(),
    })
}

/// Split an APK into its three sections for signing.
///
/// If an existing signing block is present, it is stripped (contents end before it).
/// Returns sections ready for digest computation.
pub fn split_apk(data: &[u8]) -> Result<ApkSections<'_>> {
    let (eocd_offset, cd_offset, _cd_size) = find_eocd(data)?;
    let cd_offset = cd_offset as usize;

    if cd_offset > eocd_offset || cd_offset > data.len() {
        return Err(SignError::InvalidApk {
            reason: "invalid central directory offset".into(),
        });
    }

    // Check for existing signing block before the central directory
    let contents_end = find_signing_block_start(data, cd_offset).unwrap_or(cd_offset);

    Ok(ApkSections {
        contents: &data[..contents_end],
        central_dir: &data[cd_offset..eocd_offset],
        eocd: &data[eocd_offset..],
        cd_offset: cd_offset as u32,
    })
}

/// Try to find the start of an existing APK signing block.
/// Returns None if no signing block is present.
fn find_signing_block_start(data: &[u8], cd_offset: usize) -> Option<usize> {
    // The signing block ends immediately before the central directory.
    // It ends with: [8 bytes size] [16 bytes magic]
    if cd_offset < 24 {
        return None;
    }

    let magic_start = cd_offset - 16;
    if &data[magic_start..cd_offset] != APK_SIG_BLOCK_MAGIC {
        return None;
    }

    let block_size = u64::from_le_bytes([
        data[magic_start - 8],
        data[magic_start - 7],
        data[magic_start - 6],
        data[magic_start - 5],
        data[magic_start - 4],
        data[magic_start - 3],
        data[magic_start - 2],
        data[magic_start - 1],
    ]) as usize;

    // block_size includes the trailing size field (8 bytes) but not the leading one
    // Total block = 8 (leading size) + block_size
    let block_start = cd_offset.checked_sub(8 + block_size)?;

    // Verify the leading size matches
    if block_start + 8 > data.len() {
        return None;
    }
    let leading_size = u64::from_le_bytes([
        data[block_start],
        data[block_start + 1],
        data[block_start + 2],
        data[block_start + 3],
        data[block_start + 4],
        data[block_start + 5],
        data[block_start + 6],
        data[block_start + 7],
    ]) as usize;

    if leading_size != block_size {
        return None;
    }

    Some(block_start)
}

/// Build an APK Signing Block from a list of ID-value pairs.
pub fn build_signing_block(pairs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    // Calculate total size of ID-value pairs
    let pairs_size: usize = pairs
        .iter()
        .map(|(_, v)| 8 + 4 + v.len()) // 8 (length u64) + 4 (id u32) + value
        .sum();

    // block_size = pairs + 8 (trailing size) + 16 (magic)
    let block_size = pairs_size + 8 + 16;

    let mut block = Vec::with_capacity(8 + block_size);

    // Leading size
    block.extend_from_slice(&(block_size as u64).to_le_bytes());

    // ID-value pairs
    for (id, value) in pairs {
        let pair_len = (4 + value.len()) as u64;
        block.extend_from_slice(&pair_len.to_le_bytes());
        block.extend_from_slice(&id.to_le_bytes());
        block.extend_from_slice(value);
    }

    // Trailing size
    block.extend_from_slice(&(block_size as u64).to_le_bytes());

    // Magic
    block.extend_from_slice(APK_SIG_BLOCK_MAGIC);

    block
}

/// Reassemble a signed APK from its parts.
///
/// Inserts the signing block between contents and central directory,
/// and patches the EOCD to point to the new central directory offset.
pub fn reassemble_apk(
    contents: &[u8],
    signing_block: &[u8],
    central_dir: &[u8],
    eocd: &[u8],
) -> Result<Vec<u8>> {
    let new_cd_offset = contents.len() + signing_block.len();

    let mut output = Vec::with_capacity(
        contents.len() + signing_block.len() + central_dir.len() + eocd.len(),
    );

    output.extend_from_slice(contents);
    output.extend_from_slice(signing_block);
    output.extend_from_slice(central_dir);

    // Patch EOCD with new CD offset
    let mut patched_eocd = eocd.to_vec();
    if patched_eocd.len() >= 22 {
        let offset_bytes = (new_cd_offset as u32).to_le_bytes();
        patched_eocd[16] = offset_bytes[0];
        patched_eocd[17] = offset_bytes[1];
        patched_eocd[18] = offset_bytes[2];
        patched_eocd[19] = offset_bytes[3];
    }
    output.extend_from_slice(&patched_eocd);

    Ok(output)
}
