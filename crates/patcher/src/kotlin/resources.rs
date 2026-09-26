// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `resources.arsc` entries and the global string pool. `component` is a
//! split name; `None` means the base.

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{ResValue, ResourceTable};

use super::files::{inject, with_component};
use super::handles::{bundle_path, with_ctx};
use super::types::{ResourceRef, StyleItem};
use crate::context::PatchContext;
use reseam_apk::Compression;

/// Runs `f` on the named component's table, reporting load failures to the patch.
fn with_resources<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceTable) -> R,
) -> Result<R, String> {
    with_resources_result(component, |resources| Ok(f(resources)))
}

/// Runs a fallible resource operation in the selected component.
fn with_resources_result<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    with_component(component, |ctx, index| resources_of(ctx, index, f))
        .unwrap_or_else(|| Err("unknown component".to_string()))
}

/// Runs a read-only resource operation without marking the table dirty.
fn with_resources_read<R>(
    component: Option<String>,
    f: impl FnOnce(&ResourceTable) -> R,
) -> Result<R, String> {
    with_resources_read_result(component, |resources| Ok(f(resources)))
}

/// Runs a fallible read-only operation without marking the table dirty.
fn with_resources_read_result<R>(
    component: Option<String>,
    f: impl FnOnce(&ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    with_component(component, |ctx, index| resources_of_read(ctx, index, f))
        .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_component_names() -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .components()
            .iter()
            .filter(|c| c.has_resources())
            .map(|c| c.name().to_string())
            .collect()
    })
}

/// The component defining `res_type/res_name`, searching all of them.
#[export]
pub fn res_component_for(res_type: String, res_name: String) -> Result<Option<String>, String> {
    with_ctx(|ctx| {
        let found = ctx
            .apk_mut()
            .find_resource(&res_type, &res_name)
            .map_err(|error| error.to_string())?;
        Ok(found.and_then(|(index, _)| ctx.apk().component(index).map(|c| c.name().to_string())))
    })
}

#[export]
pub fn res_component_for_id(res_id: u32) -> Result<Option<String>, String> {
    with_ctx(|ctx| {
        let found = ctx
            .apk_mut()
            .find_resource_by_id(res_id)
            .map_err(|error| error.to_string())?;
        Ok(found.and_then(|index| ctx.apk().component(index).map(|c| c.name().to_string())))
    })
}

/// The id of `res_type/res_name`; without a component every one is searched.
#[export]
pub fn res_id(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<u32>, String> {
    match component {
        None => with_ctx(|ctx| {
            ctx.apk_mut()
                .find_resource(&res_type, &res_name)
                .map_err(|error| error.to_string())
                .map(|found| found.map(|(_, id)| id))
        }),
        Some(_) => with_resources_read_result(component, |resources| {
            resources
                .find_resource_id_checked(&res_type, &res_name)
                .map_err(|error| error.to_string())
        }),
    }
}

#[export]
pub fn res_exists(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<bool, String> {
    Ok(res_id(component, res_type, res_name)?.is_some())
}

#[export]
pub fn res_get_string(component: Option<String>, name: String) -> Result<Option<String>, String> {
    match component {
        None => with_ctx(|ctx| {
            ctx.apk_mut()
                .string_resource(&name)
                .map_err(|error| error.to_string())
        }),
        Some(_) => with_resources_read_result(component, |resources| {
            resources
                .string_value_checked(&name)
                .map(|value| value.map(|text| text.into_owned()))
                .map_err(|error| error.to_string())
        }),
    }
}

#[export]
pub fn res_set_string(
    component: Option<String>,
    name: String,
    value: String,
) -> Result<bool, String> {
    match component {
        None => with_ctx(|ctx| {
            ctx.apk_mut()
                .set_string_resource(&name, &value)
                .map_err(|error| error.to_string())
        }),
        Some(_) => with_resources_result(component, |resources| {
            resources
                .set_string_value_checked(&name, &value)
                .map_err(|error| error.to_string())
        }),
    }
}

/// Adds `res_type/name` with `value` read the way resource XML is: booleans,
/// integers, colors, dimensions and `@type/name` references. A `string`
/// entry keeps the text as is.
#[export]
pub fn res_add(
    component: Option<String>,
    res_type: String,
    name: String,
    value: String,
) -> Result<Option<u32>, String> {
    with_resources_result(component, |res| {
        if res_type == "string" {
            return res
                .add_string_resource_checked(&name, &value)
                .map_err(|error| error.to_string());
        }
        match axml::parse_attribute_value(&value, None, Some(res)) {
            Ok(AttributeValue::Value(parsed)) => res
                .add_resource_checked(&res_type, &name, parsed)
                .map_err(|error| error.to_string()),
            Ok(AttributeValue::Text) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    })
}

#[export]
pub fn res_add_id(component: Option<String>, name: String) -> Result<Option<u32>, String> {
    with_resources_result(component, |res| {
        res.ensure_id_checked(&name)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_add_raw(
    component: Option<String>,
    res_type: String,
    name: String,
    data_type: u8,
    data: u32,
) -> Result<Option<u32>, String> {
    with_resources_result(component, |res| {
        res.add_resource_checked(&res_type, &name, ResValue::new(data_type, data))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_get_raw(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<i64>, String> {
    let component = match component {
        Some(component) => component,
        None => match res_component_for(res_type.clone(), res_name.clone())? {
            Some(component) => component,
            None => return Ok(None),
        },
    };
    with_resources_read_result(Some(component), |resources| {
        resources
            .resource_value_checked(&res_type, &res_name)
            .map(|value| value.map(|value| value.data as i64))
            .map_err(|error| error.to_string())
    })
}

/// Runs `f` on the component's resource table, failing if it is missing or unreadable.
fn resources_of<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&mut ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    match ctx
        .component_mut(index)
        .and_then(|c| Ok(c.resources_mut()?))
    {
        Ok(Some(resources)) => f(resources),
        Ok(None) => Err("the component has no resource table".to_string()),
        Err(error) => Err(format!("resources: {error}")),
    }
}

/// Reads a component's table without marking it for serialization.
fn resources_of_read<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    match ctx.component_mut(index).and_then(|c| Ok(c.resources()?)) {
        Ok(Some(resources)) => f(resources),
        Ok(None) => Err("the component has no resource table".to_string()),
        Err(error) => Err(format!("resources: {error}")),
    }
}

/// APK paths for every configuration of a file resource, default first.
#[export]
pub fn res_file_paths(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Vec<String>, String> {
    let component = match component {
        Some(component) => Some(component),
        None => res_component_for(res_type.clone(), res_name.clone())?,
    };
    with_component(component, |ctx, index| {
        let paths = resources_of_read(ctx, index, |resources| {
            resources
                .file_paths(&res_type, &res_name)
                .map_err(|error| error.to_string())
        })?;
        let component = ctx.apk().component(index).ok_or("unknown component")?;
        match paths.iter().find(|path| !component.contains(path)) {
            Some(missing) => Err(format!(
                "{res_type}/{res_name} is not file-backed: its value {missing} is not an entry of the APK"
            )),
            None => Ok(paths),
        }
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

/// The default configuration's path, which is the file aapt built from
/// `res/<type>/<name>` with no qualifier.
#[export]
pub fn res_file_path(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<String, String> {
    Ok(res_file_paths(component, res_type, res_name)?.swap_remove(0))
}

/// Registers an APK entry as `res_type/name` in `qualifiers` (empty for default).
#[export]
pub fn res_add_file(
    component: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        if !ctx
            .apk()
            .component(index)
            .is_some_and(|component| component.contains(&apk_path))
        {
            return Err(format!(
                "{res_type}/{name}: the APK has no entry {apk_path} to register"
            ));
        }
        resources_of(ctx, index, |resources| {
            resources
                .add_file_resource(&res_type, &name, &apk_path, &qualifiers)
                .map_err(|error| format!("{res_type}/{name}: {error}"))
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

/// Writes `data` to `apk_path` and registers it as `res_type/name` in
/// `qualifiers`. XML is compiled on the way, and each `<aapt:attr>` in it
/// becomes a resource of its own beside the file, as aapt builds them.
#[export]
pub fn res_add_file_data(
    component: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    data: Vec<u8>,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        let data = match std::str::from_utf8(&data) {
            Ok(text) if apk_path.ends_with(".xml") && !axml::is_compiled_axml(&data) => {
                let (text, inline) = axml::extract_inline_resources(text, &name, &res_type)
                    .map_err(|error| format!("{res_type}/{name}: {error}"))?;
                let dir = apk_path.rsplit_once('/').map_or("", |(dir, _)| dir);
                for resource in inline {
                    let path = format!("{dir}/{}.xml", resource.name);
                    write_file_resource(
                        ctx,
                        index,
                        (&res_type, &resource.name),
                        &path,
                        resource.xml.into_bytes(),
                        &qualifiers,
                    )?;
                }
                text.into_bytes()
            }
            _ => data,
        };
        write_file_resource(ctx, index, (&res_type, &name), &apk_path, data, &qualifiers)
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

fn write_file_resource(
    ctx: &mut PatchContext<'_>,
    index: usize,
    (res_type, name): (&str, &str),
    apk_path: &str,
    data: Vec<u8>,
    qualifiers: &str,
) -> Result<u32, String> {
    ctx.inject_file(index, apk_path, data, Compression::Deflated)
        .map_err(|error| format!("{res_type}/{name}: {error}"))?;
    resources_of(ctx, index, |resources| {
        resources
            .add_file_resource(res_type, name, apk_path, qualifiers)
            .map_err(|error| format!("{res_type}/{name}: {error}"))
    })
}

/// Renames the component's resource package, which by-name lookups against the
/// installed package name match.
#[export]
pub fn res_set_package_name(component: Option<String>, name: String) -> Result<(), String> {
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_package_name(&name)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

/// Adds or replaces `items` in `style/name`, creating the style with `parent`
/// when the table has none.
#[export]
pub fn res_style_set(
    component: Option<String>,
    name: String,
    parent: Option<String>,
    items: Vec<StyleItem>,
) -> Result<u32, String> {
    let items: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.name, item.value))
        .collect();
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_style_items(&name, parent.as_deref(), &items)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_array_get(component: Option<String>, name: String) -> Result<Vec<String>, String> {
    with_component(component, |ctx, index| {
        resources_of_read(ctx, index, |resources| {
            resources.array(&name).map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_array_set(
    component: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_array(&name, &values)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_string_array_set(
    component: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_string_array(&name, &values)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_copy(bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(None, &apk_path, data, Compression::Deflated),
        Err(error) => with_ctx(|ctx| {
            ctx.log().warn(format!(
                "res_copy: failed to read {}: {error}",
                source.display()
            ))
        }),
    }
}

/// Copies `resources/<res_type>/<file>` from the bundle into `res/<res_type>/`.
#[export]
pub fn res_copy_group(res_type: String, files: Vec<String>) {
    let bundle_dir = bundle_path("");
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    with_ctx(|ctx| {
        if let Err(error) = ctx.copy_resource_group(&bundle_dir, &res_type, &files) {
            ctx.log().warn(format!("res_copy_group: {error}"));
        }
    });
}

#[export]
pub fn res_inject(apk_path: String, data: Vec<u8>) {
    inject(None, &apk_path, data, Compression::Deflated);
}

#[export]
pub fn res_delete(apk_path: String) {
    super::files::file_delete(None, apk_path);
}

#[export]
pub fn res_list(prefix: String) -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .entry_names()
            .iter()
            .filter(|name| name.as_str().starts_with(&prefix))
            .map(ToString::to_string)
            .collect()
    })
}

#[export]
pub fn res_pool_get(component: Option<String>, index: u32) -> Result<Option<String>, String> {
    with_resources_read(component, |res| {
        res.get_string(index).map(|s| s.into_owned())
    })
}

#[export]
pub fn res_pool_set(component: Option<String>, index: u32, value: String) -> Result<(), String> {
    with_resources(component, |res| res.set_string(index, value))
}

#[export]
pub fn res_pool_add(component: Option<String>, value: String) -> Result<u32, String> {
    with_resources(component, |res| res.add_global_string(&value))
}

#[export]
pub fn res_pool_find_refs(
    component: Option<String>,
    string_index: u32,
) -> Result<Vec<ResourceRef>, String> {
    with_resources_read(component, |res| {
        res.find_entries_by_string(string_index)
            .into_iter()
            .map(|entry| ResourceRef {
                res_id: entry.res_id,
                key_name: entry.key_name,
            })
            .collect()
    })
}

/// Points a string entry at another pool string; without a component the
/// entry's own component is used.
#[export]
pub fn res_replace_entry(
    component: Option<String>,
    res_id: u32,
    new_string_index: u32,
) -> Result<(), String> {
    let component = match component {
        Some(component) => component,
        None => res_component_for_id(res_id)?
            .ok_or_else(|| format!("no component defines resource 0x{res_id:08x}"))?,
    };
    with_resources(Some(component), |res| {
        res.replace_entry_string(res_id, new_string_index)
    })
}
