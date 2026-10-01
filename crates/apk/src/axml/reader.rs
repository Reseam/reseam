// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_storage::Bytes;

use super::{
    AxmlAttribute, AxmlDocument, AxmlEvent, CHUNK_END_ELEMENT, CHUNK_END_NAMESPACE,
    CHUNK_RESOURCE_IDS, CHUNK_START_ELEMENT, CHUNK_START_NAMESPACE, CHUNK_TEXT, CHUNK_XML_DOCUMENT,
    NONE, NodeMetadata,
};
use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::chunk;
use crate::error::{Result, invalid, malformed};
use crate::string_pool::{CHUNK_STRING_POOL, StringPool};
use crate::value::ResValue;

pub(super) const ATTRIBUTE_LEN: usize = 20;

impl AxmlDocument {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::parse_bytes(Bytes::from_slice(data))
    }

    /// Parses owned or mapped bytes, retaining opaque chunks and node metadata
    /// against the same backing storage. String encodings are validated before
    /// exposing the document's infallible text queries.
    pub fn parse_bytes(bytes: Bytes) -> Result<Self> {
        let data = bytes.as_bytes();
        require_len(data, 0, chunk::HEADER_LEN, "axml document")?;
        let kind = read_u16_le(data, 0, "axml document")?;
        if kind != CHUNK_XML_DOCUMENT {
            return Err(invalid(
                "axml document",
                format!("expected XML document, got 0x{kind:04x}"),
            ));
        }
        let header_size = read_u16_le(data, 2, "axml document")? as usize;
        let end = chunk::chunk_end(data, 0)?;
        if header_size < chunk::HEADER_LEN || header_size > end {
            return Err(invalid("axml document", "invalid header size"));
        }
        let mut string_pool = None;
        let mut resource_ids = Vec::new();
        let mut resource_header = Vec::new();
        let mut elements = Vec::new();
        let mut last = header_size;
        for chunk in chunk::chunks(data, header_size..end, "axml chunk")? {
            last = chunk.range.end;
            let body = &data[chunk.range.clone()];
            let metadata = NodeMetadata {
                data: bytes.clone(),
                range: chunk.range.clone(),
            };
            let at = chunk.header_size;
            elements.push(match chunk.kind {
                CHUNK_STRING_POOL if string_pool.is_none() => {
                    string_pool = Some(StringPool::parse(&bytes, chunk.range)?);
                    AxmlEvent::StringPool
                }
                CHUNK_RESOURCE_IDS if resource_header.is_empty() => {
                    resource_ids = (at..body.len())
                        .step_by(4)
                        .map(|pos| read_u32_le(body, pos, "axml resource ids"))
                        .collect::<Result<_>>()?;
                    resource_header = body[..at].to_vec();
                    AxmlEvent::ResourceMap
                }
                _ => parse_node(chunk.kind, body, at, metadata)?,
            });
        }
        if last < end {
            elements.push(AxmlEvent::Opaque(NodeMetadata {
                data: bytes.clone(),
                range: last..end,
            }));
        }
        let string_pool =
            string_pool.ok_or_else(|| invalid("axml document", "no string pool found"))?;
        for string in string_pool.iter() {
            string?;
        }
        let header = data[..header_size].to_vec();
        let len = data.len();
        Ok(Self {
            string_pool,
            resource_ids,
            elements,
            header,
            resource_header,
            suffix: NodeMetadata {
                data: bytes,
                range: end..len,
            },
        })
    }
}

fn parse_node(kind: u16, body: &[u8], at: usize, metadata: NodeMetadata) -> Result<AxmlEvent> {
    Ok(match kind {
        CHUNK_START_NAMESPACE | CHUNK_END_NAMESPACE => {
            node_header(body, at)?;
            require_len(body, at, 8, "axml namespace")?;
            let prefix = optional(read_u32_le(body, at, "axml namespace")?);
            let uri = read_u32_le(body, at + 4, "axml namespace")?;
            if kind == CHUNK_START_NAMESPACE {
                AxmlEvent::StartNamespace {
                    metadata,
                    prefix,
                    uri,
                }
            } else {
                AxmlEvent::EndNamespace {
                    metadata,
                    prefix,
                    uri,
                }
            }
        }
        CHUNK_START_ELEMENT => parse_start_element(body, at, metadata)?,
        CHUNK_END_ELEMENT => {
            node_header(body, at)?;
            require_len(body, at, 8, "axml end element")?;
            AxmlEvent::EndElement {
                metadata,
                namespace: optional(read_u32_le(body, at, "axml end element")?),
                name: read_u32_le(body, at + 4, "axml end element")?,
            }
        }
        CHUNK_TEXT => {
            node_header(body, at)?;
            require_len(body, at, 12, "axml text")?;
            AxmlEvent::Text {
                metadata,
                text: read_u32_le(body, at, "axml text")?,
                value: ResValue::read(body, at + 4, "axml text")?,
            }
        }
        _ => AxmlEvent::Opaque(metadata),
    })
}

fn node_header(body: &[u8], header_size: usize) -> Result<()> {
    if header_size < 16 {
        return Err(invalid("axml node", "header is shorter than 16 bytes"));
    }
    require_len(body, 0, header_size, "axml node")
}

fn parse_start_element(
    body: &[u8],
    header_size: usize,
    metadata: NodeMetadata,
) -> Result<AxmlEvent> {
    node_header(body, header_size)?;
    require_len(body, header_size, ATTRIBUTE_LEN, "axml start element")?;
    let namespace = optional(read_u32_le(body, header_size, "axml start element")?);
    let name = read_u32_le(body, header_size + 4, "axml start element")?;
    let attr_start = read_u16_le(body, header_size + 8, "axml start element")? as usize;
    let attr_size = match read_u16_le(body, header_size + 10, "axml start element")? as usize {
        0 => ATTRIBUTE_LEN,
        size if size < ATTRIBUTE_LEN => {
            return Err(malformed(
                "axml start element",
                metadata.range.start + header_size + 10,
                "attribute stride is smaller than 20 bytes",
            ));
        }
        size => size,
    };
    let attr_count = read_u16_le(body, header_size + 12, "axml start element")? as usize;
    let attrs_offset = header_size + attr_start;
    let attrs_len = attr_count
        .checked_mul(attr_size)
        .ok_or_else(|| invalid("axml attributes", "attribute data size overflows"))?;
    require_len(body, attrs_offset, attrs_len, "axml attributes")?;
    let roles = [14, 16, 18]
        .map(|offset| read_u16_le(body, header_size + offset, "axml attribute index"))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let attributes = (0..attr_count)
        .map(|i| {
            let at = attrs_offset + i * attr_size;
            let mut attr = AxmlAttribute::new(
                optional(read_u32_le(body, at, "axml attribute")?),
                read_u32_le(body, at + 4, "axml attribute")?,
                ResValue::read(body, at + 12, "axml attribute")?,
            );
            attr.raw_value = optional(read_u32_le(body, at + 8, "axml attribute")?);
            attr.encoded = body[at..at + attr_size].to_vec();
            attr.roles = std::array::from_fn(|role| roles[role] as usize == i + 1);
            Ok(attr)
        })
        .collect::<Result<_>>()?;
    Ok(AxmlEvent::StartElement {
        metadata,
        namespace,
        name,
        attributes,
    })
}

fn optional(value: u32) -> Option<u32> {
    (value != NONE).then_some(value)
}
