// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod component;
mod dex_workers;
mod open;
pub(crate) use open::validate_components;
mod entries;
mod presentation;
mod write;

use std::borrow::Cow;
use std::collections::HashSet;

use reseam_dex::{DexFile, MultiDexContainer};

use crate::entry::EntryName;
use crate::error::{Result, invalid};
use crate::resources::{ResourceScope, ResourceTable, first_found};

pub use component::{ApkComponent, Compression};
pub use presentation::{ApplicationIcon, IconLayer};
pub use write::{ApkWriteOptions, SignaturePolicy};

/// Owns a base APK and its splits, parsed documents and staged entry changes.
/// Input files must remain unchanged while this file-backed session lives.
/// Component and DEX indices stay stable; deleted DEX entries leave empty slots.
pub struct ApkFile {
    options: reseam_dex::ParseOptions,
    components: Vec<ApkComponent>,
    dex: MultiDexContainer,
    dex_origins: Vec<DexOrigin>,
    pub(crate) scratch: Option<reseam_storage::ScratchDir>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ComponentIndex(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DexIndex(usize);

struct DexOrigin {
    component: ComponentIndex,
    name: EntryName,
    kind: DexSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DexSource {
    Archive,
    Added,
    Removed,
}

impl ApkFile {
    pub fn components(&self) -> &[ApkComponent] {
        &self.components
    }

    pub fn component(&self, index: usize) -> Option<&ApkComponent> {
        self.components.get(index)
    }

    pub fn component_mut(&mut self, index: usize) -> Option<&mut ApkComponent> {
        self.components.get_mut(index)
    }

    pub fn component_by_name(&self, name: &str) -> Option<usize> {
        self.components
            .iter()
            .position(|component| component.name() == name)
    }

    pub fn base(&self) -> &ApkComponent {
        &self.components[0]
    }

    pub fn base_mut(&mut self) -> &mut ApkComponent {
        &mut self.components[0]
    }

    /// The sum of every component's manifest revision.
    pub fn manifest_revision(&self) -> u64 {
        self.components
            .iter()
            .map(ApkComponent::manifest_revision)
            .sum()
    }

    pub fn package_name(&self) -> Option<Cow<'_, str>> {
        self.base().manifest().package_name()
    }

    pub fn version_code(&self) -> Option<u32> {
        self.base().manifest().version_code()
    }

    pub fn version_name(&self) -> Option<Cow<'_, str>> {
        self.base().manifest().version_name()
    }

    /// Every entry across all components, base first, without duplicates.
    pub fn entry_names(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.components
            .iter()
            .flat_map(ApkComponent::entry_names)
            .filter(|name| seen.insert(name.clone()))
            .collect()
    }

    /// DEX slots in load order, including empty tombstones left by deletion.
    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    /// One DEX without resolving deferred class data, for whole-DEX
    /// operations such as interning or adding classes.
    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        if self.dex_origins.get(index)?.kind == DexSource::Removed {
            return None;
        }
        self.dex.dex_mut(index)
    }

    /// Adds a DEX to the base component under the next unused classes name.
    /// Names stay reserved after deletion, and DEX indices remain stable.
    /// Serialization assigns additions after any overflow parts of earlier DEX
    /// files, so their output names can differ from their session entry names.
    pub fn add_dex(&mut self, dex: DexFile) {
        let mut used = self.components[0]
            .reserved_names()
            .map(EntryName::from)
            .collect();
        let name = crate::entry::next_free_dex_name(&mut used);
        self.components[0].add_dex_entry(name.as_str());
        self.dex.add_dex(dex);
        self.dex_origins.push(DexOrigin {
            component: ComponentIndex(0),
            name,
            kind: DexSource::Added,
        });
    }

    pub fn is_added_dex(&self, index: usize) -> bool {
        self.dex_origins
            .get(index)
            .is_some_and(|origin| origin.kind == DexSource::Added)
    }

    pub fn resolve_dex_class_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        if self
            .dex_origins
            .get(index)
            .is_none_or(|origin| origin.kind == DexSource::Removed)
        {
            return Ok(None);
        }
        Ok(self.dex.dex_class_resolved_mut(index, class_idx)?)
    }

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
