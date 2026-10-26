// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resource lookups across the components' tables. The platform merges every
//! split's table under one package, so a name can live in any of them.

use std::borrow::Cow;

use super::{ApkComponent, ApkFile};
use crate::error::{Result, invalid};
use crate::resources::{ResourceScope, ResourceTable, first_found};

impl ApkFile {
    pub fn find_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<(usize, u32)>> {
        self.find_in_resources(|table| table.find_resource_id(type_name, entry_name))
    }

    /// Runs `f` on component `index`'s table as a [`ResourceScope`] over the
    /// other components, whose tables load only for a name it lacks. `f` gets
    /// `None` when the component has no table.
    pub fn with_resource_scope<R>(
        &mut self,
        index: usize,
        f: impl FnOnce(Option<&mut ResourceScope<'_>>) -> R,
    ) -> Result<R> {
        if index >= self.components.len() {
            return Err(invalid("apk", format!("no component at index {index}")));
        }
        let (before, rest) = self.components.split_at_mut(index);
        let (own, after) = rest
            .split_first_mut()
            .ok_or_else(|| invalid("apk", format!("no component at index {index}")))?;
        let mut splits = |type_name: &str, entry_name: &str| {
            find_in(before.iter_mut().chain(after.iter_mut()), |table| {
                table.find_resource_id(type_name, entry_name)
            })
            .map(|found| found.map(|(_, res_id)| res_id))
        };
        Ok(match own.resources_mut()? {
            Some(table) => f(Some(&mut ResourceScope::new(table, &mut splits))),
            None => f(None),
        })
    }

    pub fn find_resource_by_id(&mut self, res_id: u32) -> Result<Option<usize>> {
        self.find_in_resources(|table| {
            table
                .contains_resource_id(res_id)
                .map(|found| found.then_some(()))
        })
        .map(|found| found.map(|(index, ())| index))
    }

    pub fn string_resource(&mut self, name: &str) -> Result<Option<String>> {
        self.find_in_resources(|table| {
            table
                .string_value(name)
                .map(|value| value.map(Cow::into_owned))
        })
        .map(|found| found.map(|(_, value)| value))
    }

    pub fn string_resource_by_id(&mut self, res_id: u32) -> Result<Option<String>> {
        self.find_in_resources(|table| {
            table
                .string_value_by_id(res_id)
                .map(|value| value.map(Cow::into_owned))
        })
        .map(|found| found.map(|(_, value)| value))
    }

    fn find_in_resources<T>(
        &mut self,
        find: impl FnMut(&ResourceTable) -> Result<Option<T>>,
    ) -> Result<Option<(usize, T)>> {
        find_in(self.components.iter_mut(), find)
    }

    /// Sets the string resource where it is defined, or in the base when it
    /// is not defined anywhere.
    pub fn set_string_resource(&mut self, name: &str, value: &str) -> Result<bool> {
        let index = self
            .find_resource("string", name)?
            .map_or(0, |(index, _)| index);
        self.components[index]
            .resources_mut()?
            .map(|resources| resources.set_string_value(name, value))
            .transpose()
            .map(|changed| changed.unwrap_or(false))
    }

    pub fn set_string_resource_by_id(&mut self, res_id: u32, value: &str) -> Result<bool> {
        let Some(index) = self.find_resource_by_id(res_id)? else {
            return Ok(false);
        };
        self.components[index]
            .resources_mut()?
            .map(|resources| resources.set_string_value_by_id(res_id, value))
            .transpose()
            .map(|changed| changed.unwrap_or(false))
    }
}

fn find_in<'c, T>(
    components: impl Iterator<Item = &'c mut ApkComponent>,
    mut find: impl FnMut(&ResourceTable) -> Result<Option<T>>,
) -> Result<Option<(usize, T)>> {
    first_found(components.enumerate().map(|(position, component)| {
        component
            .resources()
            .and_then(|resources| resources.map_or(Ok(None), &mut find))
            .map(|found| found.map(|value| (position, value)))
    }))
}
