// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Attribute names bound to the resource id the inflater looks them up by.

use super::reference::android_resource;
use super::{ANDROID_NS, AxmlAttribute, AxmlDocument};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;
use crate::resources::ResourceTable;

/// The id of attribute `local` in namespace `uri`: the framework table for
/// `android:`, `resources` for any other namespace, none without one.
pub(super) fn attribute_resource_id(
    uri: Option<&str>,
    local: &str,
    resources: Option<&ResourceTable>,
) -> Result<Option<u32>> {
    match uri {
        None => Ok(None),
        Some(ANDROID_NS) => android_resource("attr", local).map(Some),
        Some(_) => resources
            .map(|table| table.find_resource_id("attr", local))
            .transpose()?
            .flatten()
            .map(Some)
            .ok_or_else(|| invalid("XML attribute", format!("the app has no attr/{local}"))),
    }
}

impl AxmlDocument {
    /// Returns namespace and name indices, binding prefixed names to resource ids
    /// against the namespaces active at element `index`. `android:` uses the
    /// framework table; other prefixes use `resources`. Fails for unresolved
    /// prefixed names. Unqualified names have no resource id.
    pub fn bind_attribute_name_at(
        &mut self,
        index: usize,
        name: &str,
        resources: Option<&ResourceTable>,
    ) -> Result<(Option<u32>, u32)> {
        let Some((prefix, local)) = name.split_once(':') else {
            return Ok((None, self.intern_string(name)));
        };
        let namespace = self
            .declared_namespace_at(index, prefix)
            .or_else(|| (prefix == "android").then(|| self.intern_string(ANDROID_NS)))
            .ok_or_else(|| {
                invalid(
                    "axml attribute",
                    format!(
                        "attribute {name}: the document declares no xmlns:{prefix}; declare it with declareNamespace(\"{prefix}\", uri)"
                    ),
                )
            })?;
        let uri = self
            .string(namespace)
            .ok_or_else(|| invalid("XML namespace", "invalid URI index"))?;
        let res_id = attribute_resource_id(Some(&uri), local, resources)?.ok_or_else(|| {
            invalid(
                "XML attribute",
                format!("attribute {name} has no resource ID"),
            )
        })?;
        Ok((Some(namespace), self.intern_attribute_name(local, res_id)))
    }

    /// The pool index of attribute name `name` bound to `res_id`, reusing a
    /// string already bound to it.
    pub(super) fn intern_attribute_name(&mut self, name: &str, res_id: u32) -> u32 {
        let existing = self
            .resource_ids
            .iter()
            .enumerate()
            .find_map(|(index, &id)| {
                (id == res_id && self.string(index as u32).as_deref() == Some(name))
                    .then_some(index as u32)
            });
        let index = existing.unwrap_or_else(|| self.string_pool.push(name));
        self.bind_resource_id(index, res_id);
        index
    }

    /// An `android:` attribute named `name` with framework id `res_id`.
    pub fn make_attribute(&mut self, name: &str, res_id: u32, value: ResValue) -> AxmlAttribute {
        let name_index = self.intern_attribute_name(name, res_id);
        AxmlAttribute::new(self.android_ns(), name_index, value)
    }

    pub fn make_string_attribute(&mut self, name: &str, res_id: u32, value: &str) -> AxmlAttribute {
        let value = ResValue::string(self.intern_string(value));
        self.make_attribute(name, res_id, value)
    }

    fn bind_resource_id(&mut self, name: u32, res_id: u32) {
        let index = name as usize;
        if self.resource_ids.len() <= index {
            self.resource_ids.resize(index + 1, 0);
        }
        self.resource_ids[index] = res_id;
    }
}
