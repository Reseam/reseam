// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::read_u16;
use std::borrow::Cow;

use super::{finalize_node, node_bytes};
use crate::axml::document::AttributeRoles;
use crate::axml::{ATTRIBUTE_LEN, AxmlAttribute, CHUNK_START_ELEMENT, NONE, NodeMetadata};
use crate::error::{Result, invalid};

/// A start element, its original bytes when nothing about it changed.
pub(super) fn encode_start_element<'a>(
    metadata: &'a NodeMetadata,
    namespace: Option<u32>,
    name: u32,
    attributes: &[AxmlAttribute],
) -> Result<Cow<'a, [u8]>> {
    let (mut bytes, at) = node_bytes(metadata, CHUNK_START_ELEMENT, 20);
    let original = metadata.bytes();
    let attr_start = if original.is_empty() {
        20
    } else {
        read_u16(original, at + 8, "axml attribute start")? as usize
    };
    let raw_stride = if original.is_empty() {
        ATTRIBUTE_LEN
    } else {
        read_u16(original, at + 10, "axml attribute stride")? as usize
    };
    let stride = attributes
        .iter()
        .map(|attr| attr.encoded.len())
        .max()
        .unwrap_or(ATTRIBUTE_LEN)
        .max(raw_stride)
        .max(ATTRIBUTE_LEN);
    let count = if original.is_empty() {
        0
    } else {
        read_u16(original, at + 12, "axml attribute count")? as usize
    };
    let mut encoded = Vec::with_capacity(attributes.len() * stride);
    for attr in attributes {
        encode_attribute(&mut encoded, attr, stride);
    }
    let original_attrs = original.get(at + attr_start..at + attr_start + count * stride);
    if !original.is_empty()
        && original_attrs == Some(encoded.as_slice())
        && original[at..at + 4] == namespace.unwrap_or(NONE).to_le_bytes()
        && original[at + 4..at + 8] == name.to_le_bytes()
    {
        return Ok(Cow::Borrowed(original));
    }
    if attr_start < 20 {
        return Err(invalid(
            "axml element",
            "cannot edit an overlapping attribute extension",
        ));
    }
    let suffix = original
        .get(at + attr_start + count * stride..)
        .unwrap_or_default();
    bytes.truncate(at + attr_start);
    bytes.resize(at + attr_start, 0);
    bytes[at..at + 4].copy_from_slice(&namespace.unwrap_or(NONE).to_le_bytes());
    bytes[at + 4..at + 8].copy_from_slice(&name.to_le_bytes());
    bytes[at + 8..at + 10].copy_from_slice(&(attr_start as u16).to_le_bytes());
    bytes[at + 10..at + 12].copy_from_slice(
        &(if stride == raw_stride.max(ATTRIBUTE_LEN) {
            raw_stride as u16
        } else {
            u16::try_from(stride)
                .map_err(|_| invalid("axml attribute", "attribute stride exceeds 65535 bytes"))?
        })
        .to_le_bytes(),
    );
    bytes[at + 12..at + 14].copy_from_slice(
        &u16::try_from(attributes.len())
            .map_err(|_| invalid("axml element", "more than 65535 attributes"))?
            .to_le_bytes(),
    );
    for (role, offset) in AttributeRoles::FIELDS {
        let index = attributes
            .iter()
            .position(|attr| attr.roles.contains(role))
            .map_or(0, |i| i as u16 + 1);
        bytes[at + offset..at + offset + 2].copy_from_slice(&index.to_le_bytes());
    }
    bytes.extend(encoded);
    bytes.extend_from_slice(suffix);
    Ok(Cow::Owned(finalize_node(bytes)?))
}

fn encode_attribute(out: &mut Vec<u8>, attr: &AxmlAttribute, stride: usize) {
    let start = out.len();
    out.extend_from_slice(&attr.encoded);
    out.resize(start + stride, 0);
    let bytes = &mut out[start..];
    bytes[..4].copy_from_slice(&attr.namespace.unwrap_or(NONE).to_le_bytes());
    bytes[4..8].copy_from_slice(&attr.name.to_le_bytes());
    bytes[8..12].copy_from_slice(&attr.raw_value.unwrap_or(NONE).to_le_bytes());
    if attr.encoded.is_empty() {
        bytes[12..20].copy_from_slice(&attr.value.encoded());
    } else {
        attr.value.replace_payload(&mut bytes[12..20]);
    }
}
