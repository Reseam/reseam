// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml::android_attrs::{
    ATTR_CONFIG_CHANGES, ATTR_ENABLED, ATTR_LABEL, ATTR_MIME_TYPE, ATTR_NAME, ATTR_TARGET_ACTIVITY,
    android_attr_res_id,
};
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{AxmlDocument, ResValue, ResourceScope};

use super::files::with_component;
use super::handles::record_failure;
use super::xml::{self, Nodes};

fn read<R>(component: Option<String>, f: impl FnOnce(&AxmlDocument) -> R) -> Option<R> {
    with_component(component, |ctx, index| {
        f(ctx
            .apk()
            .component(index)
            .expect("component checked by with_component")
            .manifest())
    })
}

fn edit(
    component: Option<String>,
    edit: impl FnOnce(&mut AxmlDocument, &mut Nodes) -> Result<(), String>,
) {
    with_component(component, |ctx, index| {
        if let Err(message) = xml::edit_manifest(ctx, index, edit) {
            record_failure(&message);
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_package_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.package_name().map(Cow::into_owned)).flatten()
}

#[export]
pub fn manifest_version_code(component: Option<String>) -> Option<u32> {
    read(component, AxmlDocument::version_code).flatten()
}

#[export]
pub fn manifest_version_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.version_name().map(Cow::into_owned)).flatten()
}

#[export]
pub fn manifest_min_sdk_version(component: Option<String>) -> Option<u32> {
    read(component, AxmlDocument::min_sdk_version).flatten()
}

#[export]
pub fn manifest_split_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.split_name().map(Cow::into_owned)).flatten()
}

#[export]
pub fn manifest_set_version_code(component: Option<String>, code: u32) {
    edit(component, |m, _| {
        m.set_version_code(code)
            .then_some(())
            .ok_or("versionCode attribute not found".into())
    });
}

#[export]
pub fn manifest_set_version_name(component: Option<String>, name: String) {
    edit(component, |m, _| {
        m.set_version_name(&name)
            .then_some(())
            .ok_or("versionName attribute not found".into())
    });
}

#[export]
pub fn manifest_set_min_sdk(component: Option<String>, sdk: u32) {
    edit(component, |m, _| {
        m.set_min_sdk(sdk)
            .then_some(())
            .ok_or("uses-sdk minSdkVersion not found".into())
    });
}

#[export]
pub fn manifest_add_permission(component: Option<String>, permission: String) {
    edit(component, |m, nodes| {
        let end = m
            .root()
            .and_then(|root| m.find_end_element(root))
            .ok_or("manifest root not found")?;
        if !m.add_permission(&permission) {
            return Err("unterminated manifest root".into());
        }
        nodes.inserted_document(end, 2);
        Ok(())
    });
}

#[export]
pub fn manifest_set_attribute_int(
    component: Option<String>,
    element_name: String,
    attr_name: String,
    value: i32,
) {
    set_attribute(component, &element_name, &attr_name, |_| {
        ResValue::int(value)
    });
}

#[export]
pub fn manifest_set_attribute_string(
    component: Option<String>,
    element_name: String,
    attr_name: String,
    value: String,
) {
    let value = match text_value(&value, android_attr_res_id(&attr_name)) {
        Ok(value) => value,
        Err(message) => return warn(component, message),
    };
    set_attribute(component, &element_name, &attr_name, |m| value.resolve(m));
}

enum TextValue {
    Resolved(ResValue),
    Text(String),
}

impl TextValue {
    fn resolve(self, m: &mut AxmlDocument) -> ResValue {
        match self {
            TextValue::Resolved(value) => value,
            TextValue::Text(text) => ResValue::string(m.intern_string(&text)),
        }
    }
}

fn text_value(text: &str, attr: Option<u32>) -> Result<TextValue, String> {
    if !text.starts_with(['@', '?']) {
        return text_value_in(text, attr, None);
    }
    super::handles::with_ctx_result(|ctx| {
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
    match match attr {
        Some(id) => axml::parse_attribute_value(text, id, resources),
        None => axml::infer_value(text, resources),
    } {
        Ok(AttributeValue::Value(value)) => Ok(TextValue::Resolved(value)),
        Ok(AttributeValue::Text) => Err(format!("{text}: no such resource")),
        Err(error) => Err(format!("{text}: {error}")),
    }
}

fn warn(component: Option<String>, message: String) {
    record_failure(&message);
    with_component(component, |ctx, _| ctx.log().warn(message));
}

fn set_attribute(
    component: Option<String>,
    element_name: &str,
    attr_name: &str,
    value: impl FnOnce(&mut AxmlDocument) -> ResValue,
) {
    edit(component, |m, _| {
        let res_id = android_attr_res_id(attr_name)
            .ok_or_else(|| format!("unknown android attribute '{attr_name}'"))?;
        let element = m
            .find_element(element_name)
            .ok_or_else(|| format!("element '{element_name}' not found"))?;
        let value = value(m);
        if !m.set_attribute(element, res_id, value) {
            let attr = m.make_attribute(attr_name, res_id, value);
            m.add_attribute(element, attr);
        }
        Ok(())
    });
}

fn find_activity(m: &AxmlDocument, name: &str) -> Result<usize, String> {
    m.find_element_with_attr("activity", ATTR_NAME, name)
        .ok_or_else(|| format!("activity '{name}' not found"))
}

fn find_activity_or_alias(m: &AxmlDocument, name: &str) -> Result<usize, String> {
    m.find_element_with_attr("activity", ATTR_NAME, name)
        .or_else(|| m.find_element_with_attr("activity-alias", ATTR_NAME, name))
        .ok_or_else(|| format!("activity or activity-alias '{name}' not found"))
}

#[export]
pub fn manifest_set_activity_config_changes(
    component: Option<String>,
    activity_name: String,
    config_changes: String,
) {
    with_component(component, |ctx, index| {
        let outcome = xml::edit_manifest(ctx, index, |m, _| {
            let activity = find_activity(m, &activity_name)?;
            let (flags, unknown) = parse_config_changes(&config_changes)?;
            let value = ResValue::int(flags);
            if !m.set_attribute(activity, ATTR_CONFIG_CHANGES, value) {
                let attribute = m.make_attribute("configChanges", ATTR_CONFIG_CHANGES, value);
                m.add_attribute(activity, attribute);
            }
            Ok::<_, String>(unknown)
        });
        match outcome {
            Ok(unknown) if !unknown.is_empty() => ctx.log().warn(format!(
                "unknown configChanges flags: {}",
                unknown.join(", ")
            )),
            Ok(_) => {}
            Err(error) => {
                record_failure(&error);
                ctx.log().warn(error);
            }
        }
    });
}

#[export]
pub fn manifest_add_intent_filter(
    component: Option<String>,
    activity_name: String,
    action: Option<String>,
    category: Option<String>,
    mime_type: Option<String>,
) {
    edit(component, |m, nodes| {
        let activity = find_activity(m, &activity_name)?;
        m.insert_child_element(activity, "intent-filter", Vec::new())
            .map_err(|error| error.to_string())?;
        nodes.inserted_document(activity + 1, 2);
        let filter = activity + 1;
        for (element, attr, res_id, value) in [
            ("data", "mimeType", ATTR_MIME_TYPE, mime_type),
            ("category", "name", ATTR_NAME, category),
            ("action", "name", ATTR_NAME, action),
        ] {
            if let Some(value) = value {
                let attr = m.make_string_attribute(attr, res_id, &value);
                m.insert_child_element(filter, element, vec![attr])
                    .map_err(|error| error.to_string())?;
                nodes.inserted_document(filter + 1, 2);
            }
        }
        Ok(())
    });
}

#[export]
pub fn manifest_add_activity_alias(
    component: Option<String>,
    target_activity: String,
    alias_name: String,
    enabled: bool,
    label: Option<String>,
) {
    let label = match label
        .as_deref()
        .map(|label| text_value(label, Some(ATTR_LABEL)))
        .transpose()
    {
        Ok(label) => label,
        Err(message) => return warn(component, message),
    };
    edit(component, |m, nodes| {
        let application = m
            .find_element("application")
            .ok_or("application element not found")?;
        let at = m
            .find_end_element(application)
            .ok_or("unterminated application element")?;
        add_activity_alias(m, &target_activity, &alias_name, enabled, label)?;
        nodes.inserted_document(at, 2);
        Ok(())
    });
}

fn add_activity_alias(
    m: &mut AxmlDocument,
    target_activity: &str,
    alias_name: &str,
    enabled: bool,
    label: Option<TextValue>,
) -> Result<(), String> {
    let application = m
        .find_element("application")
        .ok_or("application element not found")?;
    let mut attrs = vec![
        m.make_string_attribute("name", ATTR_NAME, alias_name),
        m.make_string_attribute("targetActivity", ATTR_TARGET_ACTIVITY, target_activity),
        m.make_attribute("enabled", ATTR_ENABLED, ResValue::boolean(enabled)),
    ];
    if let Some(label) = label {
        let value = label.resolve(m);
        attrs.push(m.make_attribute("label", ATTR_LABEL, value));
    }
    if m.append_child_element(application, "activity-alias", attrs) {
        Ok(())
    } else {
        Err("unterminated application element".to_string())
    }
}

#[export]
pub fn manifest_copy_intent_filters(
    component: Option<String>,
    from_activity: String,
    to_activity: String,
) {
    edit(component, |m, nodes| {
        copy_intent_filters(m, nodes, &from_activity, &to_activity)
    });
}

fn copy_intent_filters(
    m: &mut AxmlDocument,
    nodes: &mut Nodes,
    from_activity: &str,
    to_activity: &str,
) -> Result<(), String> {
    let from = find_activity_or_alias(m, from_activity)?;
    let to = find_activity_or_alias(m, to_activity)?;
    let from_end = m
        .find_end_element(from)
        .ok_or("unterminated source activity")?;
    let mut filters = Vec::new();
    let mut i = from + 1;
    while i < from_end {
        let is_filter = m.element_name(i).as_deref() == Some("intent-filter");
        match m.find_end_element(i).filter(|_| is_filter) {
            Some(end) => {
                filters.push(m.events()[i..=end].to_vec());
                i = end + 1;
            }
            None => i += 1,
        }
    }
    let insert_at = to + 1;
    for events in filters.into_iter().rev() {
        let count = events.len();
        m.insert_events(insert_at, events)
            .map_err(|error| error.to_string())?;
        nodes.inserted_document(insert_at, count);
    }
    Ok(())
}

#[export]
pub fn manifest_get_document(component: Option<String>) -> Option<u32> {
    with_component(component, |_, index| {
        super::handles::checked(xml::open_manifest(index).map(Some))
    })
    .flatten()
}

fn parse_config_changes(text: &str) -> Result<(i32, Vec<String>), String> {
    let mut flags = 0;
    let mut unknown = Vec::new();
    for part in text.split('|').map(str::trim) {
        if matches!(
            part,
            "mcc"
                | "mnc"
                | "locale"
                | "touchscreen"
                | "keyboard"
                | "keyboardHidden"
                | "navigation"
                | "orientation"
                | "screenLayout"
                | "uiMode"
                | "screenSize"
                | "smallestScreenSize"
                | "density"
                | "layoutDirection"
                | "colorMode"
                | "fontScale"
        ) {
            let value = axml::attribute_symbols(ATTR_CONFIG_CHANGES, part, None)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("missing framework configChanges flag {part}"))?;
            flags |= value.data as i32;
        } else {
            unknown.push(part.to_owned());
        }
    }
    Ok((flags, unknown))
}
