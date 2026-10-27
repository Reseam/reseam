// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_apk::axml;
use reseam_apk::axml::android_attrs::{
    ATTR_CONFIG_CHANGES, ATTR_ENABLED, ATTR_LABEL, ATTR_MIME_TYPE, ATTR_NAME, ATTR_TARGET_ACTIVITY,
    android_attr_symbol,
};
use reseam_apk::{AxmlDocument, ResValue};

use super::components::with_component;
use super::manifest::edit;
use super::manifest_text::text_value;
use super::xml::{self, Nodes};
use crate::bridge::run_state::{checked, record_failure};

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
    checked(with_component(component, |ctx, index| {
        let unknown = xml::edit_manifest(ctx, index, |m, _| {
            let activity = find_activity(m, &activity_name)?;
            let (flags, unknown) = parse_config_changes(&config_changes);
            m.put_android_attribute(
                activity,
                "configChanges",
                ATTR_CONFIG_CHANGES,
                ResValue::int(flags),
            );
            Ok::<_, String>(unknown)
        })?;
        if !unknown.is_empty() {
            ctx.log().warn(format!(
                "unknown configChanges flags: {}",
                unknown.join(", ")
            ));
        }
        Ok(())
    }));
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
        Err(message) => return record_failure(message),
    };
    edit(component, |m, nodes| {
        let application = m
            .find_element("application")
            .ok_or("application element not found")?;
        let at =
            axml::element_end(m.events(), application).ok_or("unterminated application element")?;
        let mut attrs = vec![
            m.make_string_attribute("name", ATTR_NAME, &alias_name),
            m.make_string_attribute("targetActivity", ATTR_TARGET_ACTIVITY, &target_activity),
            m.make_attribute("enabled", ATTR_ENABLED, ResValue::boolean(enabled)),
        ];
        if let Some(label) = label {
            let value = label.resolve(m);
            attrs.push(m.make_attribute("label", ATTR_LABEL, value));
        }
        m.append_child_element(application, "activity-alias", attrs);
        nodes.inserted_document(at, 2);
        Ok(())
    });
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
    axml::element_end(m.events(), from).ok_or("unterminated source activity")?;
    let filters: Vec<_> = axml::children(m.events(), from)
        .into_iter()
        .filter(|&child| m.element_name(child).as_deref() == Some("intent-filter"))
        .filter_map(|child| {
            Some(m.events()[child..=axml::element_end(m.events(), child)?].to_vec())
        })
        .collect();
    let insert_at = to + 1;
    for events in filters.into_iter().rev() {
        let count = events.len();
        m.insert_events(insert_at, events)
            .map_err(|error| error.to_string())?;
        nodes.inserted_document(insert_at, count);
    }
    Ok(())
}

/// The `android:configChanges` mask of `|`-joined flag names, and the names
/// the framework attribute does not define.
fn parse_config_changes(text: &str) -> (i32, Vec<String>) {
    let mut flags = 0;
    let mut unknown = Vec::new();
    for part in text.split('|').map(str::trim) {
        match android_attr_symbol(ATTR_CONFIG_CHANGES, part) {
            Some(symbol) => flags |= symbol.value as i32,
            None => unknown.push(part.to_owned()),
        }
    }
    (flags, unknown)
}
