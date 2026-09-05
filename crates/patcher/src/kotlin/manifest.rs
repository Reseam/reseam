// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `AndroidManifest.xml` edits. `component` is a split name; `None` means
//! the base. While a patch holds the manifest open as an XML document, edits
//! go to that document so both views stay consistent.

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml::android_attrs::{
    android_attr_res_id, ATTR_CONFIG_CHANGES, ATTR_ENABLED, ATTR_LABEL, ATTR_MIME_TYPE, ATTR_NAME,
    ATTR_TARGET_ACTIVITY,
};
use reseam_apk::{AxmlDocument, ResValue};

use super::files::with_component;
use super::xml::{self, DocSource};
use crate::context::PatchContext;

fn read<R>(component: Option<String>, f: impl FnOnce(&AxmlDocument) -> R) -> Option<R> {
    with_component(component, |ctx, index| with_manifest(ctx, index, f))
}

/// Applies `edit` to the manifest; a returned message is logged as a warning.
fn edit(component: Option<String>, edit: impl FnOnce(&mut AxmlDocument) -> Result<(), String>) {
    with_component(component, |ctx, index| {
        let outcome = with_manifest_mut(ctx, index, edit);
        if let Err(message) = outcome {
            ctx.log().warn(message);
        }
    });
}

fn with_manifest<R>(
    ctx: &mut PatchContext<'_>,
    component: usize,
    f: impl FnOnce(&AxmlDocument) -> R,
) -> R {
    let source = DocSource::Manifest { component };
    if xml::is_open(&source) {
        return xml::with_source_doc(&source, f).expect("document is open");
    }
    f(ctx
        .apk()
        .component(component)
        .map_or(ctx.apk().base(), |c| c)
        .manifest())
}

fn with_manifest_mut<R>(
    ctx: &mut PatchContext<'_>,
    component: usize,
    f: impl FnOnce(&mut AxmlDocument) -> R,
) -> R {
    let source = DocSource::Manifest { component };
    if xml::is_open(&source) {
        return xml::with_source_doc_mut(&source, f).expect("document is open");
    }
    f(ctx
        .apk_mut()
        .component_mut(component)
        .expect("component checked by caller")
        .manifest_mut())
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
    edit(component, |m| {
        m.set_version_code(code)
            .then_some(())
            .ok_or("versionCode attribute not found".into())
    });
}

#[export]
pub fn manifest_set_version_name(component: Option<String>, name: String) {
    edit(component, |m| {
        m.set_version_name(&name)
            .then_some(())
            .ok_or("versionName attribute not found".into())
    });
}

#[export]
pub fn manifest_set_min_sdk(component: Option<String>, sdk: u32) {
    edit(component, |m| {
        m.set_min_sdk(sdk)
            .then_some(())
            .ok_or("uses-sdk minSdkVersion not found".into())
    });
}

#[export]
pub fn manifest_add_permission(component: Option<String>, permission: String) {
    edit(component, |m| {
        m.add_permission(&permission)
            .then_some(())
            .ok_or("manifest root not found".into())
    });
}

/// Sets the `android:` attribute `attr_name` on the first element named
/// `element_name`, adding it when the element lacks it.
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
    set_attribute(component, &element_name, &attr_name, |m| {
        ResValue::string(m.intern_string(&value))
    });
}

fn set_attribute(
    component: Option<String>,
    element_name: &str,
    attr_name: &str,
    value: impl FnOnce(&mut AxmlDocument) -> ResValue,
) {
    edit(component, |m| {
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

#[export]
pub fn manifest_set_activity_config_changes(
    component: Option<String>,
    activity_name: String,
    config_changes: String,
) {
    edit(component, |m| {
        let activity = find_activity(m, &activity_name)?;
        let (flags, unknown) = parse_config_changes(&config_changes);
        if !m.set_attribute(activity, ATTR_CONFIG_CHANGES, ResValue::int(flags)) {
            let attr = m.make_attribute("configChanges", ATTR_CONFIG_CHANGES, ResValue::int(flags));
            m.add_attribute(activity, attr);
        }
        match unknown.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "unknown configChanges flags: {}",
                unknown.join(", ")
            )),
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
    edit(component, |m| {
        let activity = find_activity(m, &activity_name)?;
        m.insert_child_element(activity, "intent-filter", Vec::new());
        let filter = activity + 1;
        for (element, attr, res_id, value) in [
            ("data", "mimeType", ATTR_MIME_TYPE, mime_type),
            ("category", "name", ATTR_NAME, category),
            ("action", "name", ATTR_NAME, action),
        ] {
            if let Some(value) = value {
                let attr = m.make_string_attribute(attr, res_id, &value);
                m.insert_child_element(filter, element, vec![attr]);
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
    edit(component, |m| {
        let application = m
            .find_element("application")
            .ok_or("application element not found")?;
        let mut attrs = vec![
            m.make_string_attribute("name", ATTR_NAME, &alias_name),
            m.make_string_attribute("targetActivity", ATTR_TARGET_ACTIVITY, &target_activity),
            m.make_attribute("enabled", ATTR_ENABLED, ResValue::boolean(enabled)),
        ];
        if let Some(label) = label {
            attrs.push(m.make_string_attribute("label", ATTR_LABEL, &label));
        }
        m.insert_child_element(application, "activity-alias", attrs);
        Ok(())
    });
}

/// Copies every `intent-filter` of `from_activity` to the start of `to_activity`.
#[export]
pub fn manifest_copy_intent_filters(
    component: Option<String>,
    from_activity: String,
    to_activity: String,
) {
    edit(component, |m| {
        let from = find_activity(m, &from_activity)?;
        let to = find_activity(m, &to_activity)?;
        let from_end = m
            .find_end_element(from)
            .ok_or("unterminated source activity")?;
        let mut filters = Vec::new();
        let mut i = from + 1;
        while i < from_end {
            let is_filter = m.element_name(i).as_deref() == Some("intent-filter");
            match m.find_end_element(i).filter(|_| is_filter) {
                Some(end) => {
                    filters.push(m.elements[i..=end].to_vec());
                    i = end + 1;
                }
                None => i += 1,
            }
        }
        let insert_at = to + 1;
        for events in filters.into_iter().rev() {
            m.elements.splice(insert_at..insert_at, events);
        }
        Ok(())
    });
}

/// Opens the manifest as an XML document; edits through either view are
/// shared until the document is closed.
#[export]
pub fn manifest_get_document(component: Option<String>) -> Option<u32> {
    with_component(component, |ctx, index| {
        let source = DocSource::Manifest { component: index };
        xml::open_source(&source, || {
            ctx.apk().component(index).map(|c| c.manifest().clone())
        })
    })
    .flatten()
}

fn parse_config_changes(text: &str) -> (i32, Vec<String>) {
    let mut flags = 0i32;
    let mut unknown = Vec::new();
    for part in text.split('|').map(str::trim) {
        flags |= match part {
            "mcc" => 0x0001,
            "mnc" => 0x0002,
            "locale" => 0x0004,
            "touchscreen" => 0x0008,
            "keyboard" => 0x0010,
            "keyboardHidden" => 0x0020,
            "navigation" => 0x0040,
            "orientation" => 0x0080,
            "screenLayout" => 0x0100,
            "uiMode" => 0x0200,
            "screenSize" => 0x0400,
            "smallestScreenSize" => 0x0800,
            "density" => 0x1000,
            "layoutDirection" => 0x2000,
            "colorMode" => 0x4000,
            "fontScale" => 0x4000_0000,
            other => {
                unknown.push(other.to_string());
                0
            }
        };
    }
    (flags, unknown)
}
