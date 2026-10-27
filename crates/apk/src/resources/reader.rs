// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{read_u16, require_len};
use reseam_storage::Bytes;

use super::{
    RES_TABLE_PACKAGE_TYPE, RES_TABLE_TYPE, ResPackage, ResourceTable, TABLE_HEADER_LEN, TableChunk,
};
use crate::chunk;
use crate::error::{Result, invalid};
use crate::string_pool::{CHUNK_STRING_POOL, StringEncoding, StringPool};

impl ResourceTable {
    pub fn parse(data: Bytes) -> Result<Self> {
        let buf = data.as_bytes();
        require_len(buf, 0, TABLE_HEADER_LEN, "resource table")?;
        let kind = read_u16(buf, 0, "resource table")?;
        if kind != RES_TABLE_TYPE {
            return Err(invalid(
                "resource table",
                format!("expected 0x0002, got 0x{kind:04x}"),
            ));
        }
        let header_size = read_u16(buf, 2, "resource table")? as usize;
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
                .unwrap_or_else(|| StringPool::new(Vec::new(), StringEncoding::Utf8)),
            packages,
            header: 0..header_size,
            suffix: end..buf.len(),
            chunks,
            data,
        })
    }
}
