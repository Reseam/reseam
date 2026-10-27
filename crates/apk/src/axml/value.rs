// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::android_attrs::{android_attr_formats, android_attr_symbol};
use super::reference::{attribute_ref, resource_ref};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;
use crate::resources::{AttrFormats, ResourceScope, ResourceTable};

/// What an attribute's text means once literals and references are parsed.
pub enum AttributeValue {
    Value(ResValue),
    /// Plain text the caller interns into its own string pool.
    Text,
}

/// Parses text as resource XML reads it. With the attribute it is written to,
/// the formats and enum or flag names the attribute declares apply: numeric or
/// boolean-looking text stays text when it permits only strings, and text it
/// cannot take is an error. Without one, a scalar or reference is inferred and
/// layout keywords keep the meaning the string-based APIs give them. References
/// resolve through the application scope; app attribute metadata stays local.
pub fn parse_value(
    text: &str,
    attr: Option<u32>,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<AttributeValue> {
    let mode = match attr {
        None => ValueMode::Inferred,
        Some(id) => ValueMode::Attribute {
            id,
            formats: attr_formats(id, resources.as_deref())?,
        },
    };
    if let ValueMode::Attribute { id, .. } = mode
        && let Some(value) =
            attribute_symbols(id, text, resources.as_deref().map(ResourceScope::table))?
    {
        return Ok(AttributeValue::Value(value));
    }
    if let Some(value) = literal(text, mode) {
        return Ok(AttributeValue::Value(value));
    }
    if let Some(id) = text
        .strip_prefix('?')
        .map(|r| attribute_ref(r, resources.as_deref().map(ResourceScope::table)))
        .transpose()?
        .flatten()
    {
        return Ok(AttributeValue::Value(ResValue::attribute(id)));
    }
    if let Some(id) = text
        .strip_prefix('@')
        .map(|r| resource_ref(r, resources))
        .transpose()?
        .flatten()
    {
        return Ok(AttributeValue::Value(ResValue::reference(id)));
    }
    match mode {
        ValueMode::Attribute { id, formats } if !formats.contains(AttrFormats::STRING) => {
            Err(invalid(
                "XML value",
                format!("{text:?} is incompatible with attribute {id:#010x} formats {formats:?}"),
            ))
        }
        _ => Ok(AttributeValue::Text),
    }
}

#[derive(Clone, Copy)]
enum ValueMode {
    Inferred,
    Attribute { id: u32, formats: AttrFormats },
}

/// The formats `attr` declares; an attribute that declares none takes any.
fn attr_formats(attr: u32, resources: Option<&ResourceScope<'_>>) -> Result<AttrFormats> {
    let formats = if attr >> 24 == 1 {
        android_attr_formats(attr)
    } else {
        resources
            .map(|scope| scope.table().attr_formats(attr))
            .transpose()?
            .flatten()
    };
    Ok(formats.unwrap_or_else(|| AttrFormats::from_bits_retain(0xffff)))
}

fn literal(text: &str, mode: ValueMode) -> Option<ResValue> {
    let accepts = |value: &ResValue| match mode {
        ValueMode::Inferred => true,
        ValueMode::Attribute { formats, .. } => formats.accepts(*value),
    };
    let inferred = matches!(mode, ValueMode::Inferred);
    match text {
        "true" => Some(ResValue::boolean(true)),
        "false" => Some(ResValue::boolean(false)),
        "match_parent" | "fill_parent" if inferred => Some(ResValue::int(-1)),
        "wrap_content" if inferred => Some(ResValue::int(-2)),
        "@null" => Some(ResValue::reference(0)),
        "@empty" if inferred => Some(ResValue::reference(0)),
        "@empty" => Some(ResValue::new(0, 1)),
        _ => None,
    }
    .filter(accepts)
    .or_else(|| ResValue::parse_color(text).filter(accepts))
    .or_else(|| ResValue::parse_dimension(text).filter(accepts))
    .or_else(|| match mode {
        ValueMode::Attribute { formats, .. } if formats.contains(AttrFormats::FRACTION) => {
            ResValue::parse_fraction(text)
        }
        _ => None,
    })
    .or_else(|| {
        text.strip_prefix("0x")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(ResValue::hex)
            .filter(accepts)
    })
    .or_else(|| text.parse::<i32>().ok().map(ResValue::int).filter(accepts))
    .or_else(|| {
        text.parse::<f32>()
            .ok()
            .map(ResValue::float)
            .filter(accepts)
    })
}

/// One enum name, or flag names joined by `|`, as the integer aapt compiles
/// them to: decimal for an enum, hexadecimal for flags.
pub fn attribute_symbols(
    attr: u32,
    text: &str,
    resources: Option<&ResourceTable>,
) -> Result<Option<ResValue>> {
    let mut symbols = Vec::new();
    for name in text.split('|').map(str::trim) {
        let symbol = match android_attr_symbol(attr, name) {
            Some(symbol) => Some(symbol),
            None => resources
                .map(|table| table.attr_symbol(attr, name))
                .transpose()?
                .flatten(),
        };
        let Some(symbol) = symbol else {
            return Ok(None);
        };
        symbols.push(symbol);
    }
    Ok(match symbols.as_slice() {
        [symbol] if !symbol.flags => Some(ResValue::int(symbol.value as i32)),
        _ if symbols.iter().all(|symbol| symbol.flags) => Some(ResValue::hex(
            symbols.iter().fold(0, |mask, symbol| mask | symbol.value),
        )),
        _ => None,
    })
}
