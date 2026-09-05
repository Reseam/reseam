// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `AndroidManifest.xml` accessors on top of the generic document.

use std::borrow::Cow;

use super::android_attrs::{
    ATTR_MIN_SDK_VERSION, ATTR_NAME, ATTR_SPLIT_NAME, ATTR_VERSION_CODE, ATTR_VERSION_NAME,
};
use super::AxmlDocument;
use crate::value::ResValue;

impl AxmlDocument {
    pub fn package_name(&self) -> Option<Cow<'_, str>> {
        self.attribute_named(self.root()?, "package")
            .and_then(|attr| self.attribute_string(attr))
    }

    pub fn version_code(&self) -> Option<u32> {
        self.attribute(self.root()?, ATTR_VERSION_CODE)?
            .value
            .as_int()
    }

    pub fn version_name(&self) -> Option<Cow<'_, str>> {
        self.attribute(self.root()?, ATTR_VERSION_NAME)
            .and_then(|attr| self.attribute_string(attr))
    }

    pub fn split_name(&self) -> Option<Cow<'_, str>> {
        let root = self.root()?;
        self.attribute(root, ATTR_SPLIT_NAME)
            .or_else(|| self.attribute_named(root, "split"))
            .and_then(|attr| self.attribute_string(attr))
    }

    pub fn min_sdk_version(&self) -> Option<u32> {
        let uses_sdk = self.find_element("uses-sdk")?;
        self.attribute(uses_sdk, ATTR_MIN_SDK_VERSION)?
            .value
            .as_int()
    }

    pub fn set_version_code(&mut self, code: u32) -> bool {
        self.root().is_some_and(|root| {
            self.set_attribute(root, ATTR_VERSION_CODE, ResValue::int(code as i32))
        })
    }

    pub fn set_version_name(&mut self, name: &str) -> bool {
        let value = ResValue::string(self.intern_string(name));
        self.root()
            .is_some_and(|root| self.set_attribute(root, ATTR_VERSION_NAME, value))
    }

    pub fn set_min_sdk(&mut self, sdk: u32) -> bool {
        self.find_element("uses-sdk").is_some_and(|element| {
            self.set_attribute(element, ATTR_MIN_SDK_VERSION, ResValue::int(sdk as i32))
        })
    }

    pub fn add_permission(&mut self, permission: &str) -> bool {
        let Some(root) = self.root() else {
            return false;
        };
        let attr = self.make_string_attribute("name", ATTR_NAME, permission);
        self.append_child_element(root, "uses-permission", vec![attr])
    }
}
