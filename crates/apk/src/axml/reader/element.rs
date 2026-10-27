// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{node_header, optional};
use crate::axml::document::AttributeRoles;
use crate::axml::{ATTRIBUTE_LEN, AxmlAttribute, AxmlEvent, NodeMetadata};
use crate::error::{Result, invalid, malformed};
use crate::res_value::ResValue;
use reseam_bytes::{read_u16, read_u32, require_len};

/// A start element and its attributes, each keeping its encoded bytes and
/// the special roles the element gives it.
pub(super) fn parse_start_element(
    body: &[u8],
    header_size: usize,
    metadata: NodeMetadata,
) -> Result<AxmlEvent> {
    node_header(body, header_size)?;
    require_len(body, header_size, ATTRIBUTE_LEN, "axml start element")?;
    let namespace = optional(read_u32(body, header_size, "axml start element")?);
    let name = read_u32(body, header_size + 4, "axml start element")?;
    let attr_start = read_u16(body, header_size + 8, "axml start element")? as usize;
    let attr_size = match read_u16(body, header_size + 10, "axml start element")? as usize {
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
    let attr_count = read_u16(body, header_size + 12, "axml start element")? as usize;
    let attrs_offset = header_size + attr_start;
    let attrs_len = attr_count
        .checked_mul(attr_size)
        .ok_or_else(|| invalid("axml attributes", "attribute data size overflows"))?;
    require_len(body, attrs_offset, attrs_len, "axml attributes")?;
    let roles = AttributeRoles::FIELDS
        .iter()
        .map(|&(role, offset)| {
            Ok((
                role,
                read_u16(body, header_size + offset, "axml attribute index")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let attributes = (0..attr_count)
        .map(|i| {
            let at = attrs_offset + i * attr_size;
            let mut attr = AxmlAttribute::new(
                optional(read_u32(body, at, "axml attribute")?),
                read_u32(body, at + 4, "axml attribute")?,
                ResValue::read(body, at + 12, "axml attribute")?,
            );
            attr.raw_value = optional(read_u32(body, at + 8, "axml attribute")?);
            attr.encoded = body[at..at + attr_size].to_vec();
            attr.roles = roles
                .iter()
                .filter(|&&(_, index)| usize::from(index) == i + 1)
                .fold(AttributeRoles::empty(), |roles, &(role, _)| roles | role);
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
