// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! An opened APK, or a base APK with its splits, as a mutable session: DEX
//! files, manifests, resource tables and loose entries can be changed, then
//! the whole set is written out.

mod component;
mod dex_workers;
mod open;
mod write;

use std::borrow::Cow;
use std::collections::HashSet;

use reseam_dex::{DexFile, MultiDexContainer};

use crate::error::Result;

pub use component::{ApkComponent, Compression};
pub use write::ApkWriteOptions;

pub struct ApkFile {
    components: Vec<ApkComponent>,
    dex: MultiDexContainer,
    dex_origins: Vec<DexOrigin>,
}

/// Where a DEX in the container came from. Whether it needs rewriting is the
/// DEX's own [`DexFile::is_dirty`].
enum DexOrigin {
    Existing { component: usize, name: String },
    Added,
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

    /// The entry from the first component that has it.
    pub fn read_entry(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        for component in &mut self.components {
            if let Some(data) = component.read_entry(name)? {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    /// One DEX without resolving deferred class data, for whole-DEX
    /// operations such as interning or adding classes.
    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.dex.dex_mut(index)
    }

    pub fn add_dex(&mut self, dex: DexFile) {
        self.dex.add_dex(dex);
        self.dex_origins.push(DexOrigin::Added);
    }

    pub fn resolve_dex_class_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        Ok(self.dex.dex_class_resolved_mut(index, class_idx)?)
    }

    pub fn find_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<(usize, u32)>> {
        for (index, component) in self.components.iter_mut().enumerate() {
            let found = component
                .resources()?
                .and_then(|resources| resources.find_resource_id(type_name, entry_name));
            if let Some(res_id) = found {
                return Ok(Some((index, res_id)));
            }
        }
        Ok(None)
    }

    pub fn find_resource_by_id(&mut self, res_id: u32) -> Result<Option<usize>> {
        for (index, component) in self.components.iter_mut().enumerate() {
            if component
                .resources()?
                .is_some_and(|resources| resources.contains_resource_id(res_id))
            {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    pub fn string_resource(&mut self, name: &str) -> Result<Option<String>> {
        for component in &mut self.components {
            let value = component
                .resources()?
                .and_then(|resources| resources.string_value(name).map(Cow::into_owned));
            if value.is_some() {
                return Ok(value);
            }
        }
        Ok(None)
    }

    /// Sets the string resource where it is defined, or in the base when it
    /// is not defined anywhere.
    pub fn set_string_resource(&mut self, name: &str, value: &str) -> Result<bool> {
        let index = self
            .find_resource("string", name)?
            .map_or(0, |(index, _)| index);
        Ok(self.components[index]
            .resources_mut()?
            .is_some_and(|resources| resources.set_string_value(name, value)))
    }
}
