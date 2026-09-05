// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_dex::file::DexBytes;

use super::{
    AxmlAttribute, AxmlDocument, AxmlEvent, CHUNK_END_ELEMENT, CHUNK_END_NAMESPACE,
    CHUNK_RESOURCE_IDS, CHUNK_START_ELEMENT, CHUNK_START_NAMESPACE, CHUNK_XML_DOCUMENT, NONE,
};
use crate::buf::{read_u16_le, read_u32_le, read_u8, require_len};
use crate::chunk::{self, Chunk};
use crate::error::{invalid, malformed, Result};
use crate::string_pool::{StringPool, CHUNK_STRING_POOL};
use crate::value::ResValue;

const ATTRIBUTE_LEN: usize = 20;

impl AxmlDocument {
    pub fn parse(data: &[u8]) -> Result<Self> {
        require_len(data, 0, chunk::HEADER_LEN, "axml document")?;
        let kind = read_u16_le(data, 0, "axml document")?;
        if kind != CHUNK_XML_DOCUMENT {
            return Err(invalid(
                "axml document",
                format!("expected XML document chunk (0x0003), got 0x{kind:04x}"),
            ));
        }
        let header_size = read_u16_le(data, 2, "axml document")? as usize;
        let bytes = DexBytes::from_vec(data.to_vec());

        let mut string_pool = None;
        let mut resource_ids = Vec::new();
        let mut elements = Vec::new();
        for Chunk {
            kind,
            header_size,
            range,
        } in chunk::chunks(data, header_size..data.len(), "axml chunk")?
        {
            let body = &data[range.clone()];
            match kind {
                CHUNK_STRING_POOL => string_pool = Some(StringPool::parse(&bytes, range)?),
                CHUNK_RESOURCE_IDS => {
                    resource_ids = (header_size..body.len())
                        .step_by(4)
                        .map(|at| read_u32_le(body, at, "axml resource ids"))
                        .collect::<Result<_>>()?;
                }
                CHUNK_START_NAMESPACE | CHUNK_END_NAMESPACE => {
                    require_len(body, header_size, 8, "axml namespace")?;
                    let prefix = optional(read_u32_le(body, header_size, "axml namespace")?);
                    let uri = read_u32_le(body, header_size + 4, "axml namespace")?;
                    elements.push(if kind == CHUNK_START_NAMESPACE {
                        AxmlEvent::StartNamespace { prefix, uri }
                    } else {
                        AxmlEvent::EndNamespace { prefix, uri }
                    });
                }
                CHUNK_START_ELEMENT => {
                    elements.push(parse_start_element(body, header_size, range.start)?)
                }
                CHUNK_END_ELEMENT => {
                    require_len(body, header_size, 8, "axml end element")?;
                    elements.push(AxmlEvent::EndElement {
                        namespace: optional(read_u32_le(body, header_size, "axml end element")?),
                        name: read_u32_le(body, header_size + 4, "axml end element")?,
                    });
                }
                _ => {}
            }
        }

        Ok(Self {
            string_pool: string_pool
                .ok_or_else(|| invalid("axml document", "no string pool found"))?,
            resource_ids,
            elements,
        })
    }
}

fn parse_start_element(body: &[u8], header_size: usize, base: usize) -> Result<AxmlEvent> {
    require_len(body, header_size, ATTRIBUTE_LEN, "axml start element")?;
    let namespace = optional(read_u32_le(body, header_size, "axml start element")?);
    let name = read_u32_le(body, header_size + 4, "axml start element")?;
    let attr_start = read_u16_le(body, header_size + 8, "axml start element")? as usize;
    let attr_size = match read_u16_le(body, header_size + 10, "axml start element")? as usize {
        0 => ATTRIBUTE_LEN,
        size if size < ATTRIBUTE_LEN => {
            return Err(malformed(
                "axml start element",
                base + header_size + 10,
                "attribute size is smaller than 20 bytes",
            ))
        }
        size => size,
    };
    let attr_count = read_u16_le(body, header_size + 12, "axml start element")? as usize;
    let attrs_offset = header_size + attr_start;
    let attrs_len = attr_count.checked_mul(attr_size).ok_or_else(|| {
        malformed(
            "axml start element",
            base + header_size + 12,
            "attribute data overflows chunk",
        )
    })?;
    require_len(body, attrs_offset, attrs_len, "axml attributes")?;

    let attributes = (0..attr_count)
        .map(|i| {
            let at = attrs_offset + i * attr_size;
            require_len(body, at, ATTRIBUTE_LEN, "axml attribute")?;
            Ok(AxmlAttribute {
                namespace: optional(read_u32_le(body, at, "axml attribute")?),
                name: read_u32_le(body, at + 4, "axml attribute")?,
                raw_value: optional(read_u32_le(body, at + 8, "axml attribute")?),
                value: ResValue::new(
                    read_u8(body, at + 15, "axml attribute")?,
                    read_u32_le(body, at + 16, "axml attribute")?,
                ),
            })
        })
        .collect::<Result<_>>()?;

    Ok(AxmlEvent::StartElement {
        namespace,
        name,
        attributes,
    })
}

fn optional(value: u32) -> Option<u32> {
    (value != NONE).then_some(value)
}
