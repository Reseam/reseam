// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod element;

use super::{
    AxmlDocument, AxmlEvent, CHUNK_END_ELEMENT, CHUNK_END_NAMESPACE, CHUNK_RESOURCE_IDS,
    CHUNK_START_ELEMENT, CHUNK_START_NAMESPACE, CHUNK_TEXT, CHUNK_XML_DOCUMENT, NONE, NodeMetadata,
};
use crate::chunk::{self, write_header};
use crate::error::{Result, invalid};
use reseam_bytes::write_u32;

impl AxmlDocument {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        let pool = self.string_pool.plan()?;
        let has_pool = self
            .elements
            .iter()
            .any(|event| matches!(event, AxmlEvent::StringPool));
        let has_ids = self
            .elements
            .iter()
            .any(|event| matches!(event, AxmlEvent::ResourceMap));
        if !has_pool {
            pool.write(&mut body)?;
        }
        if !has_ids && !self.resource_ids.is_empty() {
            self.encode_resource_ids(&mut body)?;
        }
        for event in &self.elements {
            match event {
                AxmlEvent::StringPool => pool.write(&mut body)?,
                AxmlEvent::ResourceMap => self.encode_resource_ids(&mut body)?,
                _ => encode_event(&mut body, event)?,
            }
        }
        let header_size = self.header.len().max(chunk::HEADER_LEN);
        let size = u32::try_from(header_size + body.len())
            .map_err(|_| invalid("axml document", "document exceeds 4 GiB"))?;
        let mut out = if self.header.is_empty() {
            let mut out = Vec::new();
            write_header(
                &mut out,
                CHUNK_XML_DOCUMENT,
                chunk::HEADER_LEN as u16,
                size as usize,
            );
            out
        } else {
            self.header.clone()
        };
        out[4..8].copy_from_slice(&size.to_le_bytes());
        out.extend(body);
        out.extend_from_slice(self.suffix.bytes());
        Ok(out)
    }

    fn encode_resource_ids(&self, out: &mut Vec<u8>) -> Result<()> {
        let mut header = if self.resource_header.is_empty() {
            let mut header = Vec::new();
            write_header(&mut header, CHUNK_RESOURCE_IDS, chunk::HEADER_LEN as u16, 0);
            header
        } else {
            self.resource_header.clone()
        };
        let size = u32::try_from(header.len() + self.resource_ids.len() * 4)
            .map_err(|_| invalid("axml resource map", "map exceeds 4 GiB"))?;
        header[4..8].copy_from_slice(&size.to_le_bytes());
        out.extend(header);
        for &id in &self.resource_ids {
            write_u32(out, id);
        }
        Ok(())
    }
}

pub(super) fn node_bytes(metadata: &NodeMetadata, kind: u16, body_size: usize) -> (Vec<u8>, usize) {
    let original = metadata.bytes();
    if !original.is_empty() {
        let header_size = u16::from_le_bytes([original[2], original[3]]) as usize;
        let end = if kind == CHUNK_START_ELEMENT {
            header_size
                + u16::from_le_bytes([original[header_size + 8], original[header_size + 9]])
                    as usize
        } else {
            original.len()
        };
        return (original[..end].to_vec(), header_size);
    }
    let mut bytes = Vec::new();
    write_header(&mut bytes, kind, 16, 16 + body_size);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, NONE);
    bytes.resize(16 + body_size, 0);
    (bytes, 16)
}

fn encode_event(out: &mut Vec<u8>, event: &AxmlEvent) -> Result<()> {
    let bytes = match event {
        AxmlEvent::StartNamespace {
            metadata,
            prefix,
            uri,
        }
        | AxmlEvent::EndNamespace {
            metadata,
            prefix,
            uri,
        } => {
            let kind = if matches!(event, AxmlEvent::StartNamespace { .. }) {
                CHUNK_START_NAMESPACE
            } else {
                CHUNK_END_NAMESPACE
            };
            let (mut bytes, at) = node_bytes(metadata, kind, 8);
            bytes[at..at + 4].copy_from_slice(&prefix.unwrap_or(NONE).to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&uri.to_le_bytes());
            bytes
        }
        AxmlEvent::StartElement {
            metadata,
            namespace,
            name,
            attributes,
        } => {
            let bytes = element::encode_start_element(metadata, *namespace, *name, attributes)?;
            out.extend_from_slice(&bytes);
            return Ok(());
        }

        AxmlEvent::EndElement {
            metadata,
            namespace,
            name,
        } => {
            let (mut bytes, at) = node_bytes(metadata, CHUNK_END_ELEMENT, 8);
            bytes[at..at + 4].copy_from_slice(&namespace.unwrap_or(NONE).to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&name.to_le_bytes());
            bytes
        }
        AxmlEvent::Text {
            metadata,
            text,
            value,
        } => {
            let (mut bytes, at) = node_bytes(metadata, CHUNK_TEXT, 12);
            bytes[at..at + 4].copy_from_slice(&text.to_le_bytes());
            if metadata.bytes().is_empty() {
                bytes[at + 4..at + 12].copy_from_slice(&value.encoded());
            } else {
                value.replace_payload(&mut bytes[at + 4..at + 12]);
            }
            bytes
        }
        AxmlEvent::Opaque(metadata) => {
            out.extend_from_slice(metadata.bytes());
            return Ok(());
        }
        AxmlEvent::StringPool | AxmlEvent::ResourceMap => return Ok(()),
    };
    out.extend(finalize_node(bytes)?);
    Ok(())
}

pub(super) fn finalize_node(mut bytes: Vec<u8>) -> Result<Vec<u8>> {
    let size =
        u32::try_from(bytes.len()).map_err(|_| invalid("axml node", "node exceeds 4 GiB"))?;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(bytes)
}
