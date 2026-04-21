// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::{AxmlDocument, ResourceTable};

use super::PatchContext;

impl<'a> PatchContext<'a> {
    pub fn manifest(&self) -> &AxmlDocument {
        self.apk.manifest()
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.apk.manifest_mut()
    }

    pub fn resources(&self) -> Option<&ResourceTable> {
        self.apk.resources()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        self.apk.resources_mut()
    }

    pub fn resource_component_names(&self) -> Vec<String> {
        (0..self.apk.component_count())
            .filter(|&index| self.apk.component_resources(index).is_some())
            .filter_map(|index| {
                self.apk
                    .component_meta(index)
                    .map(|component| component.name.to_string())
            })
            .collect()
    }

    pub fn resource_component_name(&self, index: usize) -> Option<&str> {
        self.apk
            .component_meta(index)
            .map(|component| component.name.as_str())
    }

    pub fn resource_component_index(&self, name: &str) -> Option<usize> {
        self.apk.component_index_by_name(name)
    }

    pub fn component_resources(&self, index: usize) -> Option<&ResourceTable> {
        self.apk.component_resources(index)
    }

    pub fn component_resources_mut(&mut self, index: usize) -> Option<&mut ResourceTable> {
        self.apk.component_resources_mut(index)
    }

    pub fn component_manifest(&self, index: usize) -> Option<&AxmlDocument> {
        self.apk.component_manifest(index)
    }

    pub fn component_manifest_mut(&mut self, index: usize) -> Option<&mut AxmlDocument> {
        self.apk.component_manifest_mut(index)
    }

    pub fn component_names(&self) -> Vec<String> {
        (0..self.apk.component_count())
            .filter_map(|index| {
                self.apk
                    .component_meta(index)
                    .map(|component| component.name.to_string())
            })
            .collect()
    }

    pub fn component_name(&self, index: usize) -> Option<&str> {
        self.apk
            .component_meta(index)
            .map(|component| component.name.as_str())
    }

    pub fn component_index(&self, name: &str) -> Option<usize> {
        self.apk.component_index_by_name(name)
    }
}
