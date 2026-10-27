// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml;
use reseam_apk::axml::android_attrs::android_attr_res_id;
use reseam_apk::{AxmlDocument, ResValue};

use super::files::with_component;
use super::handles::record_failure;
use super::manifest_text::text_value;
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

pub(super) fn edit(
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
pub fn manifest_version_code(component: Option<String>) -> Option<i32> {
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
pub fn manifest_set_version_code(component: Option<String>, code: i32) {
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
            .and_then(|root| axml::element_end(m.events(), root))
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

pub(super) fn warn(component: Option<String>, message: String) {
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
        m.put_android_attribute(element, attr_name, res_id, value);
        Ok(())
    });
}

#[export]
pub fn manifest_get_document(component: Option<String>) -> Option<u32> {
    with_component(component, |_, index| {
        super::handles::checked(xml::open_manifest(index).map(Some))
    })
    .flatten()
}
