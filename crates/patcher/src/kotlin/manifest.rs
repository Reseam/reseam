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
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{AxmlDocument, ResValue, ResourceTable};

use super::files::with_component;
use super::handles::with_ctx;
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
    let value = match text_value(&value, android_attr_res_id(&attr_name)) {
        Ok(value) => value,
        Err(message) => return warn(component, message),
    };
    set_attribute(component, &element_name, &attr_name, |m| value.resolve(m));
}

/// A manifest value given as text: `@type/name` and `?attr` are references
/// resolved against the base resource table and an enum or flag name of the
/// attribute is its value, as aapt writes them; anything else is a string,
/// with a leading `\@` or `\?` escaping the character.
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
    with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        text_value_in(text, attr, resources)
    })
}

fn text_value_in(
    text: &str,
    attr: Option<u32>,
    resources: Option<&mut ResourceTable>,
) -> Result<TextValue, String> {
    if let Some(escaped) = text
        .strip_prefix('\\')
        .filter(|rest| rest.starts_with(['@', '?']))
    {
        return Ok(TextValue::Text(escaped.to_string()));
    }
    if !text.starts_with(['@', '?']) {
        return Ok(attr
            .and_then(|attr| axml::attribute_symbols(attr, text, None))
            .map_or_else(|| TextValue::Text(text.to_string()), TextValue::Resolved));
    }
    match axml::parse_attribute_value(text, attr, resources) {
        Ok(AttributeValue::Value(value)) => Ok(TextValue::Resolved(value)),
        Ok(AttributeValue::Text) => Err(format!("{text}: no such resource")),
        Err(error) => Err(format!("{text}: {error}")),
    }
}

fn warn(component: Option<String>, message: String) {
    with_component(component, |ctx, _| ctx.log().warn(message));
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
    let label = match label
        .as_deref()
        .map(|label| text_value(label, Some(ATTR_LABEL)))
        .transpose()
    {
        Ok(label) => label,
        Err(message) => return warn(component, message),
    };
    edit(component, |m| {
        add_activity_alias(m, &target_activity, &alias_name, enabled, label)
    });
}

/// Appends the alias after every existing child of `<application>`: Android
/// rejects an alias whose target activity has not been declared above it.
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
    match m.append_child_element(application, "activity-alias", attrs) {
        true => Ok(()),
        false => Err("unterminated application element".to_string()),
    }
}

/// Copies every `intent-filter` of `from_activity` to the start of
/// `to_activity`; either may be an `<activity-alias>`.
#[export]
pub fn manifest_copy_intent_filters(
    component: Option<String>,
    from_activity: String,
    to_activity: String,
) {
    edit(component, |m| {
        copy_intent_filters(m, &from_activity, &to_activity)
    });
}

fn copy_intent_filters(
    m: &mut AxmlDocument,
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

#[cfg(test)]
mod tests {
    use reseam_apk::axml::build_document;

    use super::*;

    fn attribute_ids(m: &AxmlDocument, element: usize) -> Vec<u32> {
        m.attributes(element)
            .iter()
            .map(|attr| m.resource_id_for(attr.name).unwrap())
            .collect()
    }

    fn ascending(ids: &[u32]) -> bool {
        ids.windows(2).all(|pair| pair[0] < pair[1])
    }

    #[test]
    fn an_added_alias_follows_the_activity_it_targets_with_sorted_attributes() {
        let mut m = build_document(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="p">
  <application>
    <activity android:name=".Main"/>
    <activity-alias android:name=".Existing" android:targetActivity=".Main"/>
  </application>
</manifest>"#,
            None,
        )
        .unwrap();

        add_activity_alias(
            &mut m,
            ".Main",
            ".Added",
            false,
            Some(TextValue::Resolved(ResValue::reference(0x7f14_0001))),
        )
        .unwrap();

        let target = find_activity(&m, ".Main").unwrap();
        let existing = find_activity_or_alias(&m, ".Existing").unwrap();
        let added = find_activity_or_alias(&m, ".Added").unwrap();
        assert!(target < existing && existing < added);
        let application = m.find_element("application").unwrap();
        assert_eq!(
            m.find_end_element(added).unwrap() + 1,
            m.find_end_element(application).unwrap()
        );
        // The framework walks attributes in id order, so `enabled` (0x0e) and
        // `label` (0x01) written after `targetActivity` (0x202) were never read.
        let ids = attribute_ids(&m, added);
        assert_eq!(
            ids,
            [ATTR_LABEL, ATTR_NAME, ATTR_ENABLED, ATTR_TARGET_ACTIVITY]
        );
        let label = m.attribute(added, ATTR_LABEL).unwrap();
        assert_eq!(
            (label.value.kind, label.value.data),
            (ResValue::REFERENCE, 0x7f14_0001)
        );

        let exported = m.make_attribute(
            "exported",
            android_attr_res_id("exported").unwrap(),
            ResValue::boolean(true),
        );
        assert!(m.add_attribute(added, exported));
        assert!(ascending(&attribute_ids(&m, added)));
    }

    #[test]
    fn manifest_text_resolves_references_and_keeps_plain_text() {
        let mut table = ResourceTable::parse(reseam_apk::reseam_dex::file::DexBytes::from_vec(
            string_table("app_name"),
        ))
        .unwrap();
        let id = table.find_resource_id("string", "app_name").unwrap();

        let resolved =
            |text: &str, table: &mut ResourceTable| text_value_in(text, None, Some(table));
        match resolved("@string/app_name", &mut table).unwrap() {
            TextValue::Resolved(value) => {
                assert_eq!((value.kind, value.data), (ResValue::REFERENCE, id))
            }
            TextValue::Text(text) => panic!("stayed text: {text}"),
        }
        assert_eq!(
            resolved("@string/missing", &mut table).err().unwrap(),
            "@string/missing: invalid axml compiler: @string/missing is not defined in the resource table"
        );
        for (text, expected) in [
            ("YouTube", "YouTube"),
            ("true", "true"),
            ("\\@home", "@home"),
        ] {
            match resolved(text, &mut table).unwrap() {
                TextValue::Text(text) => assert_eq!(text, expected),
                TextValue::Resolved(_) => panic!("{text} was resolved"),
            }
        }
    }

    #[test]
    fn manifest_text_takes_the_enum_and_flag_names_of_its_attribute() {
        let value =
            |attr: &str, text: &str| match text_value_in(text, android_attr_res_id(attr), None)
                .unwrap()
            {
                TextValue::Resolved(value) => Some(value),
                TextValue::Text(_) => None,
            };
        assert_eq!(value("launchMode", "singleTask"), Some(ResValue::int(2)));
        assert_eq!(
            value("configChanges", "orientation|screenSize"),
            Some(ResValue::hex(0x80 | 0x400))
        );
        assert_eq!(value("label", "singleTask"), None);
        assert_eq!(value("launchMode", "orientation"), None);
    }

    fn string_table(name: &str) -> Vec<u8> {
        use reseam_apk::resources::{ResEntry, ResPackage, ResType, TypeSpec};
        use reseam_apk::StringPool;

        let pool =
            |items: &[&str]| StringPool::new(items.iter().map(|s| s.to_string()).collect(), true);
        let mut package = ResPackage::new(0x7F, "p", pool(&["string"]), pool(&[name]));
        package.type_specs.push(TypeSpec::new(1, vec![0]));
        let mut res_type = ResType::new(1, vec![0u8; 48]);
        res_type.push(Some(ResEntry {
            flags: 0,
            key: 0,
            value: reseam_apk::resources::EntryValue::Simple(ResValue::string(0)),
        }));
        package.types.push(res_type);
        ResourceTable {
            global_strings: pool(&["YouTube"]),
            packages: vec![package],
        }
        .serialize()
        .unwrap()
    }

    #[test]
    fn intent_filters_copy_to_and_from_activity_aliases() {
        let mut m = build_document(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="p">
  <application>
    <activity android:name=".Main">
      <intent-filter><action android:name="android.intent.action.MAIN"/></intent-filter>
      <intent-filter><action android:name="p.OPEN"/></intent-filter>
    </activity>
    <activity-alias android:name=".Alias" android:targetActivity=".Main"/>
    <activity-alias android:name=".Second" android:targetActivity=".Main"/>
  </application>
</manifest>"#,
            None,
        )
        .unwrap();

        copy_intent_filters(&mut m, ".Main", ".Alias").unwrap();
        copy_intent_filters(&mut m, ".Alias", ".Second").unwrap();

        for alias in [".Alias", ".Second"] {
            let start = find_activity_or_alias(&m, alias).unwrap();
            let end = m.find_end_element(start).unwrap();
            let filters = (start..end)
                .filter(|&i| m.element_name(i).as_deref() == Some("intent-filter"))
                .count();
            assert_eq!(filters, 2, "{alias}");
        }
        assert_eq!(
            copy_intent_filters(&mut m, ".Main", ".Missing").unwrap_err(),
            "activity or activity-alias '.Missing' not found"
        );
    }
}
