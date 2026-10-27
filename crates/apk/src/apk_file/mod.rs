// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod component;
mod dex;
mod dex_set;
mod open;
pub(crate) use open::order_components;
mod entries;
mod presentation;
mod resources;
mod undo;
mod write;

use std::borrow::Cow;
use std::collections::HashSet;

pub use component::ApkComponent;
pub use dex_set::DexSet;
pub use presentation::{ApplicationIcon, IconLayer};
pub use write::{ApkWriteOptions, SignaturePolicy};

/// Owns a base APK and its splits, parsed documents and staged entry changes.
/// Input files must remain unchanged while this file-backed session lives.
/// Component and DEX indices stay stable; deleted DEX entries leave empty slots.
pub struct ApkFile {
    options: reseam_dex::ParseOptions,
    components: Vec<ApkComponent>,
    dex: DexSet,
    /// Where the components of a container were extracted, removed with the session.
    _scratch: Option<reseam_storage::ScratchDir>,
    journal: reseam_journal::Journal<undo::Part, undo::Undo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ComponentIndex(usize);

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

    pub fn version_code(&self) -> Option<i32> {
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
}
