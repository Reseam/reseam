// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    AttrFormats, ResValue, ResourceScope, ResourceTable, Result, android_attr_symbol,
    android_resource, invalid,
};

/// What an attribute's text means once literals and references are parsed.
pub enum AttributeValue {
    Value(ResValue),
    /// Plain text the caller interns into its own string pool.
    Text,
}

/// Parses text using the formats and symbols declared by an attribute. Numeric
/// or boolean-looking text stays text when the attribute permits only strings.
/// Incompatible values and malformed definitions are errors. References resolve
/// through the application scope; app attribute metadata remains local.
pub fn parse_attribute_value(
    text: &str,
    attr: u32,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<AttributeValue> {
    let formats = if attr >> 24 == 1 {
        crate::axml::android_attrs::android_attr_formats(attr)
    } else {
        resources
            .as_deref()
            .map(|scope| scope.table().attr_formats(attr))
            .transpose()?
            .flatten()
    }
    .unwrap_or_else(|| AttrFormats::from_bits_retain(0xffff));
    parse_value(text, ValueMode::Attribute { id: attr, formats }, resources)
}

/// Infers a scalar or resource reference from text for APIs that have no
/// attribute declaration. Other text is left for the caller to intern. Layout
/// keywords retain the compatibility interpretation of the string-based APIs.
pub fn infer_value(
    text: &str,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<AttributeValue> {
    parse_value(text, ValueMode::Inferred, resources)
}

#[derive(Clone, Copy)]
enum ValueMode {
    Inferred,
    Attribute { id: u32, formats: AttrFormats },
}

fn parse_value(
    text: &str,
    mode: ValueMode,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<AttributeValue> {
    if let ValueMode::Attribute { id, .. } = mode
        && let Some(value) =
            attribute_symbols(id, text, resources.as_deref().map(ResourceScope::table))?
    {
        return Ok(AttributeValue::Value(value));
    }
    let accepts = |value: &ResValue| match mode {
        ValueMode::Inferred => true,
        ValueMode::Attribute { formats, .. } => formats.accepts(*value),
    };
    let literal = match text {
        "true" => Some(ResValue::boolean(true)),
        "false" => Some(ResValue::boolean(false)),
        "match_parent" | "fill_parent" if matches!(mode, ValueMode::Inferred) => {
            Some(ResValue::int(-1))
        }
        "wrap_content" if matches!(mode, ValueMode::Inferred) => Some(ResValue::int(-2)),
        "@null" => Some(ResValue::reference(0)),
        "@empty" if matches!(mode, ValueMode::Inferred) => Some(ResValue::reference(0)),
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
    });
    if let Some(value) = literal {
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

fn hex_ref(text: &str) -> Option<u32> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("ref/0x"))?;
    u32::from_str_radix(hex, 16).ok()
}

fn attribute_ref(text: &str, resources: Option<&ResourceTable>) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
    if let Some(name) = text.strip_prefix("android:attr/") {
        return android_resource("attr", name).map(Some);
    }
    let name = text.strip_prefix("attr/").unwrap_or(text);
    Ok(resources
        .map(|res| res.find_resource_id("attr", name))
        .transpose()?
        .flatten())
}

fn resource_ref(text: &str, resources: Option<&mut ResourceScope<'_>>) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
    let create = text.starts_with("+id/");
    let text = text.strip_prefix('+').unwrap_or(text);
    let Some((type_part, entry)) = text.split_once('/') else {
        return Ok(None);
    };
    let (namespace, type_name) = match type_part.split_once(':') {
        Some((namespace, type_name)) => (Some(namespace), type_name),
        None => (None, type_part),
    };
    if type_name.is_empty() || entry.is_empty() {
        return Ok(None);
    }
    Ok(match (namespace, resources) {
        (Some("android"), _) => Some(android_resource(type_name, entry)?),
        (Some(_), _) | (None, None) => None,
        (None, Some(res)) if create => res.ensure_id(entry)?,
        // aapt fails the build here; plain text would reach the inflater as a string and crash it.
        (None, Some(res)) => Some(res.resource_id(type_name, entry)?.ok_or_else(|| {
            invalid(
                "axml compiler",
                format!("@{type_name}/{entry} is not defined in the resource table"),
            )
        })?),
    })
}
