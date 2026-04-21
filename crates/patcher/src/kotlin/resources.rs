// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use boltffi::export;

use super::types::ResourceRef;
use super::{with_ctx, BUNDLE_DIR};

fn component_index(ctx: &crate::context::PatchContext<'_>, component: &str) -> Option<usize> {
    ctx.resource_component_index(component)
}

#[export]
pub fn res_id(res_type: String, res_name: String) -> Option<u32> {
    with_ctx(|ctx| ctx.find_resource_id(&res_type, &res_name))
}

#[export]
pub fn res_component_names() -> Vec<String> {
    with_ctx(|ctx| ctx.resource_component_names())
}

#[export]
pub fn res_component_for(res_type: String, res_name: String) -> Option<String> {
    with_ctx(|ctx| {
        let index = ctx.find_resource_component(&res_type, &res_name)?;
        ctx.resource_component_name(index).map(str::to_string)
    })
}

#[export]
pub fn res_component_for_id(res_id: u32) -> Option<String> {
    with_ctx(|ctx| {
        let index = ctx.find_resource_component_by_id(res_id)?;
        ctx.resource_component_name(index).map(str::to_string)
    })
}

#[export]
pub fn res_id_in_component(component: String, res_type: String, res_name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.find_resource_id_in_component(index, &res_type, &res_name)
    })
}

#[export]
pub fn res_exists(res_type: String, res_name: String) -> bool {
    with_ctx(|ctx| ctx.resource_exists(&res_type, &res_name))
}

#[export]
pub fn res_exists_in_component(component: String, res_type: String, res_name: String) -> bool {
    with_ctx(|ctx| {
        let Some(index) = component_index(ctx, &component) else {
            return false;
        };
        ctx.component_resources(index)
            .is_some_and(|r| r.resource_exists(&res_type, &res_name))
    })
}

#[export]
pub fn res_get_string(name: String) -> Option<String> {
    with_ctx(|ctx| ctx.get_string_resource_value(&name).map(|s| s.to_string()))
}

#[export]
pub fn res_get_string_in_component(component: String, name: String) -> Option<String> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources(index)
            .and_then(|r| r.get_string_value(&name).map(|s| s.to_string()))
    })
}

#[export]
pub fn res_set_string(name: String, value: String) -> bool {
    with_ctx(|ctx| ctx.set_string_resource_value(&name, &value))
}

#[export]
pub fn res_add_string(name: String, value: String) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_string_resource(&name, &value))
}

#[export]
pub fn res_set_string_in_component(component: String, name: String, value: String) -> bool {
    with_ctx(|ctx| {
        let Some(index) = component_index(ctx, &component) else {
            return false;
        };
        let Some(res) = ctx.component_resources_mut(index) else {
            return false;
        };
        res.set_string_value(&name, &value)
    })
}

#[export]
pub fn res_add_string_in_component(component: String, name: String, value: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_string_resource(&name, &value)
    })
}

#[export]
pub fn res_add_bool(name: String, value: bool) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_bool_resource(&name, value))
}

#[export]
pub fn res_add_bool_in_component(component: String, name: String, value: bool) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_bool_resource(&name, value)
    })
}

#[export]
pub fn res_add_integer(name: String, value: i32) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_integer_resource(&name, value))
}

#[export]
pub fn res_add_integer_in_component(component: String, name: String, value: i32) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_integer_resource(&name, value)
    })
}

#[export]
pub fn res_add_color(name: String, color: String) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_color_parsed(&name, &color))
}

#[export]
pub fn res_add_color_in_component(component: String, name: String, color: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_color_parsed(&name, &color)
    })
}

#[export]
pub fn res_add_dimen(name: String, dimen: String) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_dimen_parsed(&name, &dimen))
}

#[export]
pub fn res_add_dimen_in_component(component: String, name: String, dimen: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_dimen_parsed(&name, &dimen)
    })
}

#[export]
pub fn res_add_id(name: String) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.ensure_id(&name))
}

#[export]
pub fn res_add_id_in_component(component: String, name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?.ensure_id(&name)
    })
}

#[export]
pub fn res_copy(bundle_path: String, apk_path: String) {
    let full_path = BUNDLE_DIR.with(|bd| {
        let bd = bd.borrow();
        match bd.as_ref() {
            Some(dir) => dir.join(&bundle_path),
            None => std::path::PathBuf::from(&bundle_path),
        }
    });
    match std::fs::read(&full_path) {
        Ok(data) => with_ctx(|ctx| ctx.inject_file(&apk_path, data)),
        Err(e) => with_ctx(|ctx| {
            ctx.log().warn(format!(
                "res_copy: failed to read {}: {e}",
                full_path.display()
            ))
        }),
    }
}

#[export]
pub fn res_copy_group(res_type: String, files: Vec<String>) {
    let bundle_dir = BUNDLE_DIR.with(|bd| bd.borrow().clone());
    let bundle_dir = match bundle_dir {
        Some(dir) => dir,
        None => {
            with_ctx(|ctx| {
                ctx.log()
                    .warn("res_copy_group: bundle directory is not set".to_string())
            });
            return;
        }
    };
    for file_name in &files {
        let src = bundle_dir.join("resources").join(&res_type).join(file_name);
        match std::fs::read(&src) {
            Ok(data) => {
                let apk_path = format!("res/{res_type}/{file_name}");
                with_ctx(|ctx| ctx.inject_file(&apk_path, data));
            }
            Err(e) => with_ctx(|ctx| {
                ctx.log().warn(format!(
                    "res_copy_group: failed to read {}: {e}",
                    src.display()
                ))
            }),
        }
    }
}

#[export]
pub fn res_inject(apk_path: String, data: Vec<u8>) {
    with_ctx(|ctx| ctx.inject_file(&apk_path, data));
}

#[export]
pub fn res_delete(apk_path: String) {
    with_ctx(|ctx| ctx.delete_file(&apk_path));
}

#[export]
pub fn res_list(prefix: String) -> Vec<String> {
    with_ctx(|ctx| {
        ctx.list_files()
            .iter()
            .filter(|f| f.as_str().starts_with(&prefix))
            .map(ToString::to_string)
            .collect()
    })
}

#[export]
pub fn res_add_raw(res_type: String, name: String, data_type: u8, data: u32) -> Option<u32> {
    with_ctx(|ctx| {
        ctx.resources_mut()?
            .add_resource(&res_type, &name, data_type, data)
    })
}

#[export]
pub fn res_add_raw_in_component(
    component: String,
    res_type: String,
    name: String,
    data_type: u8,
    data: u32,
) -> Option<u32> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.component_resources_mut(index)?
            .add_resource(&res_type, &name, data_type, data)
    })
}

#[export]
pub fn res_get_raw(res_type: String, res_name: String) -> Option<i64> {
    with_ctx(|ctx| {
        let component_index = ctx.find_resource_component(&res_type, &res_name)?;
        let (_, data) = ctx
            .component_resources(component_index)?
            .get_resource_value(&res_type, &res_name)?;
        Some(data as i64)
    })
}

#[export]
pub fn res_get_raw_in_component(
    component: String,
    res_type: String,
    res_name: String,
) -> Option<i64> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        let (_, data) = ctx
            .component_resources(index)?
            .get_resource_value(&res_type, &res_name)?;
        Some(data as i64)
    })
}

#[export]
pub fn res_pool_get(index: u32) -> Option<String> {
    with_ctx(|ctx| {
        ctx.resources()
            .and_then(|r| r.get_string(index).map(|s| s.to_string()))
    })
}

#[export]
pub fn res_pool_set(index: u32, value: String) {
    with_ctx(|ctx| {
        if let Some(res) = ctx.resources_mut() {
            res.set_string(index, value);
        }
    });
}

#[export]
pub fn res_pool_add(value: String) -> Option<u32> {
    with_ctx(|ctx| Some(ctx.resources_mut()?.add_global_string(&value)))
}

#[export]
pub fn res_pool_find_refs(string_index: u32) -> Vec<ResourceRef> {
    with_ctx(|ctx| {
        let res = match ctx.resources() {
            Some(r) => r,
            None => return Vec::new(),
        };
        res.find_entries_by_string(string_index)
            .into_iter()
            .map(|e| ResourceRef {
                res_id: e.res_id,
                package_id: e.package_id as u8,
                type_id: e.type_id,
                entry_index: e.entry_index as u16,
                key_name: e.key_name,
            })
            .collect()
    })
}

#[export]
pub fn res_pool_get_in_component(component: String, index: u32) -> Option<String> {
    with_ctx(|ctx| {
        let index_component = component_index(ctx, &component)?;
        ctx.component_resources(index_component)
            .and_then(|r| r.get_string(index).map(|s| s.to_string()))
    })
}

#[export]
pub fn res_pool_set_in_component(component: String, index: u32, value: String) {
    with_ctx(|ctx| {
        if let Some(index_component) = component_index(ctx, &component) {
            if let Some(res) = ctx.component_resources_mut(index_component) {
                res.set_string(index, value);
            }
        }
    });
}

#[export]
pub fn res_pool_add_in_component(component: String, value: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index_component = component_index(ctx, &component)?;
        Some(
            ctx.component_resources_mut(index_component)?
                .add_global_string(&value),
        )
    })
}

#[export]
pub fn res_pool_find_refs_in_component(component: String, string_index: u32) -> Vec<ResourceRef> {
    with_ctx(|ctx| {
        let Some(index_component) = component_index(ctx, &component) else {
            return Vec::new();
        };
        let Some(res) = ctx.component_resources(index_component) else {
            return Vec::new();
        };
        res.find_entries_by_string(string_index)
            .into_iter()
            .map(|e| ResourceRef {
                res_id: e.res_id,
                package_id: e.package_id as u8,
                type_id: e.type_id,
                entry_index: e.entry_index as u16,
                key_name: e.key_name,
            })
            .collect()
    })
}

#[export]
pub fn res_replace_entry(res_id: u32, new_string_index: u32) {
    with_ctx(|ctx| {
        if let Some(index) = ctx.find_resource_component_by_id(res_id) {
            if let Some(res) = ctx.component_resources_mut(index) {
                res.replace_entry_string(res_id, new_string_index);
            }
        }
    });
}

#[export]
pub fn res_replace_entry_in_component(component: String, res_id: u32, new_string_index: u32) {
    with_ctx(|ctx| {
        if let Some(index) = component_index(ctx, &component) {
            if let Some(res) = ctx.component_resources_mut(index) {
                res.replace_entry_string(res_id, new_string_index);
            }
        }
    });
}
