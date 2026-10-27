// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml;
use reseam_apk::axml::android_attrs::android_attr_res_id;
use reseam_apk::{AxmlDocument, ResValue};

use super::components::{component, with_component};
use super::manifest_text::text_value;
use super::xml::{self, Nodes};
use crate::bridge::run_state::{checked, record_failure};

fn read<R: Default>(component_name: Option<String>, f: impl FnOnce(&AxmlDocument) -> R) -> R {
    checked(with_component(component_name, |ctx, index| {
        Ok(f(component(ctx, index).manifest()))
    }))
}

pub(in crate::bridge) fn edit(
    component_name: Option<String>,
    edit: impl FnOnce(&mut AxmlDocument, &mut Nodes) -> Result<(), String>,
) {
    checked(with_component(component_name, |ctx, index| {
        xml::edit_manifest(ctx, index, edit)
    }));
}

#[export]
pub fn manifest_package_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.package_name().map(Cow::into_owned))
}

#[export]
pub fn manifest_version_code(component: Option<String>) -> Option<i32> {
    read(component, AxmlDocument::version_code)
}

#[export]
pub fn manifest_version_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.version_name().map(Cow::into_owned))
}

#[export]
pub fn manifest_min_sdk_version(component: Option<String>) -> Option<u32> {
    read(component, AxmlDocument::min_sdk_version)
}

#[export]
pub fn manifest_split_name(component: Option<String>) -> Option<String> {
    read(component, |m| m.split_name().map(Cow::into_owned))
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
    match text_value(&value, android_attr_res_id(&attr_name)) {
        Ok(value) => set_attribute(component, &element_name, &attr_name, |m| value.resolve(m)),
        Err(message) => record_failure(message),
    }
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
pub fn manifest_get_document(component_name: Option<String>) -> Option<u32> {
    checked(with_component(component_name, |_, index| {
        xml::open_manifest(index).map(Some)
    }))
}
