// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::package::PackageChunk;
use super::{
    RES_TABLE_PACKAGE_TYPE, RES_TABLE_TYPE, ResPackage, ResourceTable, TABLE_HEADER_LEN, TableChunk,
};
use crate::buf::read_u32_le;
use crate::buf::{read_u16_le, require_len};
use crate::chunk;
use crate::error::{Result, invalid, malformed};
use crate::string_pool::{CHUNK_STRING_POOL, StringPool};
use reseam_storage::Bytes;

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
            if bytes.len() < 8 || read_u16_le(bytes, 0, "package chunk")? != 0x0206 {
                continue;
            }
            let header = usize::from(read_u16_le(bytes, 2, "staged aliases")?);
            require_len(bytes, 0, header.max(12), "staged aliases")?;
            if header < 12 {
                return Err(malformed(
                    "staged aliases",
                    range.start,
                    "header is shorter than 12 bytes",
                ));
            }
            let count = read_u32_le(bytes, 8, "staged aliases")? as usize;
            let size = count
                .checked_mul(8)
                .ok_or_else(|| malformed("staged aliases", range.start, "record size overflows"))?;
            require_len(bytes, header, size, "staged aliases")?;
            for record in bytes[header..header + size].as_chunks::<8>().0 {
                if u32::from_le_bytes(
                    record[..4]
                        .try_into()
                        .expect("alias record has eight bytes"),
                ) == id
                {
                    return Ok(u32::from_le_bytes(
                        record[4..]
                            .try_into()
                            .expect("alias record has eight bytes"),
                    ));
                }
            }
        }
        Ok(id)
    }

    pub fn parse(data: Bytes) -> Result<Self> {
        let buf = data.as_bytes();
        require_len(buf, 0, TABLE_HEADER_LEN, "resource table")?;
        let kind = read_u16_le(buf, 0, "resource table")?;
        if kind != RES_TABLE_TYPE {
            return Err(invalid(
                "resource table",
                format!("expected 0x0002, got 0x{kind:04x}"),
            ));
        }
        let header_size = read_u16_le(buf, 2, "resource table")? as usize;
        let end = chunk::chunk_end(buf, 0)?;
        if header_size < TABLE_HEADER_LEN || header_size > end {
            return Err(invalid("resource table", "invalid header size"));
        }
        let mut global_strings = None;
        let mut packages = Vec::new();
        let mut chunks = Vec::new();
        let mut last = header_size;
        for chunk in chunk::chunks(buf, header_size..end, "resource chunk")? {
            last = chunk.range.end;
            chunks.push(match chunk.kind {
                CHUNK_STRING_POOL if global_strings.is_none() => {
                    global_strings = Some(StringPool::parse(&data, chunk.range)?);
                    TableChunk::Strings
                }
                RES_TABLE_PACKAGE_TYPE => {
                    let index = packages.len();
                    packages.push(ResPackage::parse(&data, chunk.range, chunk.header_size)?);
                    TableChunk::Package(index)
                }
                _ => TableChunk::Raw(chunk.range),
            });
        }
        if last < end {
            chunks.push(TableChunk::Raw(last..end));
        }
        Ok(Self {
            global_strings: global_strings
                .unwrap_or_else(|| StringPool::new(Vec::new(), crate::StringEncoding::Utf8)),
            packages,
            header: 0..header_size,
            suffix: end..buf.len(),
            chunks,
            data,
        })
    }
}
