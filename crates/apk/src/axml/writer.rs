// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    AxmlAttribute, AxmlDocument, AxmlEvent, CHUNK_END_ELEMENT, CHUNK_END_NAMESPACE,
    CHUNK_RESOURCE_IDS, CHUNK_START_ELEMENT, CHUNK_START_NAMESPACE, CHUNK_XML_DOCUMENT, NONE,
};
use crate::buf::{write_u16, write_u32};
use crate::chunk::{self, write_header};
use crate::error::Result;

const NODE_HEADER_LEN: u16 = 16;
const ATTRIBUTE_LEN: usize = 20;

impl AxmlDocument {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        self.string_pool.plan().write(&mut body)?;
        if !self.resource_ids.is_empty() {
            write_header(
                &mut body,
                CHUNK_RESOURCE_IDS,
                chunk::HEADER_LEN as u16,
                chunk::HEADER_LEN + self.resource_ids.len() * 4,
            );
            for id in &self.resource_ids {
                write_u32(&mut body, *id);
            }
        }
        for event in &self.elements {
            encode_event(&mut body, event);
        }
        let mut out = Vec::with_capacity(chunk::HEADER_LEN + body.len());
        write_header(
            &mut out,
            CHUNK_XML_DOCUMENT,
            chunk::HEADER_LEN as u16,
            chunk::HEADER_LEN + body.len(),
        );
        out.extend_from_slice(&body);
        Ok(out)
    }
}

fn encode_event(out: &mut Vec<u8>, event: &AxmlEvent) {
    match event {
        AxmlEvent::StartNamespace { prefix, uri } => {
            node_header(out, CHUNK_START_NAMESPACE, 8);
            write_u32(out, prefix.unwrap_or(NONE));
            write_u32(out, *uri);
        }
        AxmlEvent::EndNamespace { prefix, uri } => {
            node_header(out, CHUNK_END_NAMESPACE, 8);
            write_u32(out, prefix.unwrap_or(NONE));
            write_u32(out, *uri);
        }
        AxmlEvent::StartElement {
            namespace,
            name,
            attributes,
        } => {
            node_header(
                out,
                CHUNK_START_ELEMENT,
                20 + attributes.len() * ATTRIBUTE_LEN,
            );
            write_u32(out, namespace.unwrap_or(NONE));
            write_u32(out, *name);
            write_u16(out, 20);
            write_u16(out, ATTRIBUTE_LEN as u16);
            write_u16(out, attributes.len() as u16);
            write_u16(out, 0);
            write_u16(out, 0);
            write_u16(out, 0);
            for attr in attributes {
                encode_attribute(out, attr);
            }
        }
        AxmlEvent::EndElement { namespace, name } => {
            node_header(out, CHUNK_END_ELEMENT, 8);
            write_u32(out, namespace.unwrap_or(NONE));
            write_u32(out, *name);
        }
    }
}

/// Chunk header plus the line number and comment every node carries.
fn node_header(out: &mut Vec<u8>, kind: u16, body_len: usize) {
    write_header(
        out,
        kind,
        NODE_HEADER_LEN,
        NODE_HEADER_LEN as usize + body_len,
    );
    write_u32(out, 0);
    write_u32(out, NONE);
}

fn encode_attribute(out: &mut Vec<u8>, attr: &AxmlAttribute) {
    write_u32(out, attr.namespace.unwrap_or(NONE));
    write_u32(out, attr.name);
    write_u32(out, attr.raw_value.unwrap_or(NONE));
    write_u16(out, 8);
    out.push(0);
    out.push(attr.value.kind);
    write_u32(out, attr.value.data);
}
