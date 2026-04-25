// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::PatchContext;

impl<'a> PatchContext<'a> {
    pub fn find_resource_id(&mut self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.apk.find_resource_id(type_name, entry_name)
    }

    pub fn find_resource_component(&mut self, type_name: &str, entry_name: &str) -> Option<usize> {
        self.apk.find_resource_component(type_name, entry_name)
    }

    pub fn find_resource_component_by_id(&mut self, res_id: u32) -> Option<usize> {
        self.apk.find_resource_component_by_id(res_id)
    }

    pub fn find_resource_id_in_component(
        &mut self,
        component_index: usize,
        type_name: &str,
        entry_name: &str,
    ) -> Option<u32> {
        self.apk
            .component_find_resource_id(component_index, type_name, entry_name)
    }

    pub fn resource_exists(&mut self, type_name: &str, entry_name: &str) -> bool {
        self.apk.resource_exists(type_name, entry_name)
    }

    pub fn get_string_resource_value(&mut self, name: &str) -> Option<&str> {
        self.apk.get_string_resource_value(name)
    }

    pub fn set_string_resource_value(&mut self, name: &str, value: &str) -> bool {
        self.apk.set_string_resource_value(name, value)
    }
}
