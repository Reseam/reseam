// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ResourceTable;
use super::package::PackageChunk;
use crate::error::{Result, malformed};
use reseam_bytes::{read_u16, read_u32, require_len};

/// `RES_TABLE_STAGED_ALIAS_TYPE`: records mapping staged ids to finalized ones.
const STAGED_ALIAS_CHUNK: u16 = 0x0206;
const HEADER_LEN: usize = 12;

impl ResourceTable {
    /// Resolves a development resource ID through this package's staged-alias
    /// metadata. Returns the original ID when no alias exists. Invalid alias
    /// headers or records are errors; all metadata remains file-backed.
    pub fn finalized_resource_id(&self, id: u32) -> Result<u32> {
        for (package, chunk) in self
            .packages
            .iter()
            .flat_map(|package| package.chunks.iter().map(move |chunk| (package, chunk)))
        {
            let PackageChunk::Raw(range) = chunk else {
                continue;
            };
            let bytes = &package.data.as_bytes()[range.clone()];
            if bytes.len() < 8 || read_u16(bytes, 0, "package chunk")? != STAGED_ALIAS_CHUNK {
                continue;
            }
            let header = usize::from(read_u16(bytes, 2, "staged aliases")?);
            require_len(bytes, 0, header.max(HEADER_LEN), "staged aliases")?;
            if header < HEADER_LEN {
                return Err(malformed(
                    "staged aliases",
                    range.start,
                    "header is shorter than 12 bytes",
                ));
            }
            let count = read_u32(bytes, 8, "staged aliases")? as usize;
            let size = count
                .checked_mul(8)
                .ok_or_else(|| malformed("staged aliases", range.start, "record size overflows"))?;
            require_len(bytes, header, size, "staged aliases")?;
            for record in bytes[header..header + size].as_chunks::<8>().0 {
                let [staged, finalized] = [&record[..4], &record[4..]]
                    .map(|half| u32::from_le_bytes(half.try_into().expect("half of eight bytes")));
                if staged == id {
                    return Ok(finalized);
                }
            }
        }
        Ok(id)
    }
}
