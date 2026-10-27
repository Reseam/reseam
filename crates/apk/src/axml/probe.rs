// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::read_u32;
use reseam_storage::Bytes;

use super::reader::document_body;
use super::{
    AxmlDocument, CHUNK_RESOURCE_IDS, CHUNK_START_ELEMENT, CHUNK_START_NAMESPACE,
    android_attr_res_id,
};
use crate::chunk;
use crate::error::{Result, invalid};
use crate::string_pool::{CHUNK_STRING_POOL, StringPool};

impl AxmlDocument {
    /// Whether a document can carry one of `names`, judged from its resource map and string pool;
    /// `true` still needs the parsed document.
    pub fn may_carry_attribute(data: &[u8], names: &[&str]) -> Result<bool> {
        let locals: Vec<&str> = names
            .iter()
            .map(|name| name.split_once(':').map_or(*name, |(_, local)| local))
            .collect();
        let ids: Vec<u32> = locals
            .iter()
            .filter_map(|local| android_attr_res_id(local))
            .collect();
        let mut pool = None;
        for chunk in chunk::chunks(data, document_body(data)?, "axml chunk")? {
            match chunk.kind {
                CHUNK_STRING_POOL if pool.is_none() => pool = Some(chunk.range),
                CHUNK_RESOURCE_IDS => {
                    let body = &data[chunk.range.clone()];
                    for pos in (chunk.header_size..body.len()).step_by(4) {
                        if ids.contains(&read_u32(body, pos, "axml resource ids")?) {
                            return Ok(true);
                        }
                    }
                }
                CHUNK_START_NAMESPACE | CHUNK_START_ELEMENT => break,
                _ => {}
            }
        }
        let range = pool.ok_or_else(|| invalid("axml document", "no string pool found"))?;
        let pool = StringPool::parse(&Bytes::from_slice(data), range)?;
        for local in locals {
            if pool.contains(local)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
