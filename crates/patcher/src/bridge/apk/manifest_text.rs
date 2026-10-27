// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{AxmlDocument, ResValue, ResourceScope};

pub(in crate::bridge) enum TextValue {
    Resolved(ResValue),
    Text(String),
}

impl TextValue {
    pub(in crate::bridge) fn resolve(self, m: &mut AxmlDocument) -> ResValue {
        match self {
            TextValue::Resolved(value) => value,
            TextValue::Text(text) => ResValue::string(m.intern_string(&text)),
        }
    }
}

/// Text as the manifest setters read it: references and the attribute's enum
/// and flag names become values, `\@` and `\?` escape a leading character, and
/// anything else stays a string.
pub(in crate::bridge) fn text_value(text: &str, attr: Option<u32>) -> Result<TextValue, String> {
    if !text.starts_with(['@', '?']) {
        return text_value_in(text, attr, None);
    }
    crate::bridge::phase::with_ctx_result(|ctx| {
        ctx.apk_mut()
            .with_resource_scope(0, |scope| text_value_in(text, attr, scope))
            .map_err(|error| error.to_string())?
    })
}

fn text_value_in(
    text: &str,
    attr: Option<u32>,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<TextValue, String> {
    if let Some(escaped) = text
        .strip_prefix('\\')
        .filter(|rest| rest.starts_with(['@', '?']))
    {
        return Ok(TextValue::Text(escaped.to_string()));
    }
    if !text.starts_with(['@', '?']) {
        return Ok(attr
            .map(|attr| axml::attribute_symbols(attr, text, None))
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten()
            .map_or_else(|| TextValue::Text(text.to_string()), TextValue::Resolved));
    }
    match axml::parse_value(text, attr, resources) {
        Ok(AttributeValue::Value(value)) => Ok(TextValue::Resolved(value)),
        Ok(AttributeValue::Text) => Err(format!("{text}: no such resource")),
        Err(error) => Err(format!("{text}: {error}")),
    }
}
