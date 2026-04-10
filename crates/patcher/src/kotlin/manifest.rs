use boltffi::export;
use stitch_apk::AxmlDocument;

use super::XML_DOCUMENTS;
use super::with_ctx;

fn manifest_slot_id(component_index: usize) -> String {
    format!("@manifest:{component_index}")
}

fn manifest_component_index(
    ctx: &crate::context::PatchContext<'_>,
    component: &str,
) -> Option<usize> {
    ctx.component_index(component)
}

fn with_manifest<R>(
    ctx: &mut crate::context::PatchContext<'_>,
    component_index: usize,
    f: impl FnOnce(&AxmlDocument) -> R,
) -> R {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let slot_id = manifest_slot_id(component_index);
        if let Some(Some((document, _))) = docs
            .iter()
            .find(|slot| matches!(slot, Some((_, path)) if path == &slot_id))
        {
            return f(document);
        }
        f(ctx.component_manifest(component_index).unwrap_or_else(|| ctx.manifest()))
    })
}

fn with_base_manifest<R>(
    ctx: &mut crate::context::PatchContext<'_>,
    f: impl FnOnce(&AxmlDocument) -> R,
) -> R {
    with_manifest(ctx, 0, f)
}

fn with_base_manifest_mut<R>(
    ctx: &mut crate::context::PatchContext<'_>,
    f: impl FnOnce(&mut AxmlDocument) -> R,
) -> R {
    with_manifest_mut(ctx, 0, f)
}

fn with_manifest_mut<R>(
    ctx: &mut crate::context::PatchContext<'_>,
    component_index: usize,
    f: impl FnOnce(&mut AxmlDocument) -> R,
) -> R {
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let slot_id = manifest_slot_id(component_index);
        if let Some(Some((document, _))) = docs
            .iter_mut()
            .find(|slot| matches!(slot, Some((_, path)) if path == &slot_id))
        {
            return f(document);
        }
        drop(docs);
        if component_index == 0 {
            f(ctx.manifest_mut())
        } else {
            if let Some(manifest) = ctx.component_manifest_mut(component_index) {
                f(manifest)
            } else {
                f(ctx.manifest_mut())
            }
        }
    })
}

#[export]
pub fn manifest_package_name() -> String {
    with_ctx(|ctx| {
        with_base_manifest(ctx, |manifest| {
            manifest.package_name().unwrap_or_default().to_string()
        })
    })
}

#[export]
pub fn manifest_version_code() -> Option<u32> {
    with_ctx(|ctx| with_base_manifest(ctx, |manifest| manifest.version_code()))
}

#[export]
pub fn manifest_version_name() -> Option<String> {
    with_ctx(|ctx| {
        with_base_manifest(ctx, |manifest| {
            manifest.version_name().map(|s| s.to_string())
        })
    })
}

#[export]
pub fn manifest_min_sdk_version() -> Option<u32> {
    with_ctx(|ctx| with_base_manifest(ctx, |manifest| manifest.min_sdk_version()))
}

#[export]
pub fn manifest_split_name() -> Option<String> {
    with_ctx(|ctx| with_base_manifest(ctx, |manifest| manifest.split_name().map(|s| s.to_string())))
}

#[export]
pub fn manifest_set_version_code(code: u32) {
    with_ctx(|ctx| with_base_manifest_mut(ctx, |manifest| manifest.set_version_code(code)));
}

#[export]
pub fn manifest_set_version_name(name: String) {
    with_ctx(|ctx| with_base_manifest_mut(ctx, |manifest| manifest.set_version_name(&name)));
}

#[export]
pub fn manifest_set_min_sdk(sdk: u32) {
    with_ctx(|ctx| with_base_manifest_mut(ctx, |manifest| manifest.set_min_sdk(sdk)));
}

#[export]
pub fn manifest_add_permission(permission: String) {
    with_ctx(|ctx| with_base_manifest_mut(ctx, |manifest| manifest.add_permission(&permission)));
}

#[export]
pub fn manifest_set_attribute_int(element_name: String, attr_res_id: u32, value: i32) {
    with_ctx(|ctx| {
        let mut warning = None;
        with_base_manifest_mut(ctx, |manifest| {
            if let Some(idx) = manifest.find_element_index(&element_name) {
                manifest.set_element_attribute_int(idx, attr_res_id, value);
            } else {
                warning = Some(format!(
                    "manifest_set_attribute_int: element '{element_name}' not found"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_set_attribute_int_in_component(
    component: String,
    element_name: String,
    attr_res_id: u32,
    value: i32,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        with_manifest_mut(ctx, index, |manifest| {
            if let Some(idx) = manifest.find_element_index(&element_name) {
                manifest.set_element_attribute_int(idx, attr_res_id, value);
            } else {
                warning = Some(format!(
                    "manifest_set_attribute_int_in_component: element '{element_name}' not found in {component}"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_set_attribute_string(element_name: String, attr_res_id: u32, value: String) {
    with_ctx(|ctx| {
        let mut warning = None;
        with_base_manifest_mut(ctx, |manifest| {
            if let Some(idx) = manifest.find_element_index(&element_name) {
                manifest.add_element_attribute_string(idx, &element_name, attr_res_id, &value);
            } else {
                warning = Some(format!(
                    "manifest_set_attribute_string: element '{element_name}' not found"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_set_attribute_string_in_component(
    component: String,
    element_name: String,
    attr_res_id: u32,
    value: String,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        with_manifest_mut(ctx, index, |manifest| {
            if let Some(idx) = manifest.find_element_index(&element_name) {
                manifest.add_element_attribute_string(idx, &element_name, attr_res_id, &value);
            } else {
                warning = Some(format!(
                    "manifest_set_attribute_string_in_component: element '{element_name}' not found in {component}"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_set_activity_config_changes(activity_name: String, config_changes: String) {
    with_ctx(|ctx| {
        let mut warning = None;
        let mut parse_warnings = Vec::new();
        with_base_manifest_mut(ctx, |manifest| {
            if let Some(idx) =
                manifest.find_element_with_attr("activity", 0x01010003, &activity_name)
            {
                let (flags, warnings) = parse_config_changes(&config_changes);
                parse_warnings = warnings;
                manifest.set_element_attribute_int(idx, 0x0101001f, flags);
            } else {
                warning = Some(format!(
                    "manifest_set_activity_config_changes: activity '{activity_name}' not found"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
        for message in parse_warnings {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_set_activity_config_changes_in_component(
    component: String,
    activity_name: String,
    config_changes: String,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        let mut parse_warnings = Vec::new();
        with_manifest_mut(ctx, index, |manifest| {
            if let Some(idx) =
                manifest.find_element_with_attr("activity", 0x01010003, &activity_name)
            {
                let (flags, warnings) = parse_config_changes(&config_changes);
                parse_warnings = warnings;
                manifest.set_element_attribute_int(idx, 0x0101001f, flags);
            } else {
                warning = Some(format!(
                    "manifest_set_activity_config_changes_in_component: activity '{activity_name}' not found in {component}"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
        for message in parse_warnings {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_add_intent_filter(
    activity_name: String,
    action: Option<String>,
    category: Option<String>,
    mime_type: Option<String>,
) {
    with_ctx(|ctx| {
        let mut warning = None;
        with_base_manifest_mut(ctx, |manifest| {
            let act_idx =
                match manifest.find_element_with_attr("activity", 0x01010003, &activity_name) {
                    Some(idx) => idx,
                    None => {
                        warning = Some(format!(
                            "manifest_add_intent_filter: activity '{activity_name}' not found"
                        ));
                        return;
                    }
                };
            manifest.insert_child_element(act_idx, "intent-filter", Vec::new());
            let filter_idx = act_idx + 1;
            if let Some(action_name) = action {
                let attr = manifest.make_string_attribute("name", 0x01010003, &action_name);
                manifest.insert_child_element(filter_idx, "action", vec![attr]);
            }
            if let Some(cat_name) = category {
                let attr = manifest.make_string_attribute("name", 0x01010003, &cat_name);
                manifest.insert_child_element(filter_idx, "category", vec![attr]);
            }
            if let Some(mime) = mime_type {
                let attr = manifest.make_string_attribute("mimeType", 0x01010026, &mime);
                manifest.insert_child_element(filter_idx, "data", vec![attr]);
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_add_intent_filter_in_component(
    component: String,
    activity_name: String,
    action: Option<String>,
    category: Option<String>,
    mime_type: Option<String>,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        with_manifest_mut(ctx, index, |manifest| {
            let act_idx =
                match manifest.find_element_with_attr("activity", 0x01010003, &activity_name) {
                    Some(idx) => idx,
                    None => {
                        warning = Some(format!(
                            "manifest_add_intent_filter_in_component: activity '{activity_name}' not found in {component}"
                        ));
                        return;
                    }
                };
            manifest.insert_child_element(act_idx, "intent-filter", Vec::new());
            let filter_idx = act_idx + 1;
            if let Some(action_name) = action {
                let attr = manifest.make_string_attribute("name", 0x01010003, &action_name);
                manifest.insert_child_element(filter_idx, "action", vec![attr]);
            }
            if let Some(cat_name) = category {
                let attr = manifest.make_string_attribute("name", 0x01010003, &cat_name);
                manifest.insert_child_element(filter_idx, "category", vec![attr]);
            }
            if let Some(mime) = mime_type {
                let attr = manifest.make_string_attribute("mimeType", 0x01010026, &mime);
                manifest.insert_child_element(filter_idx, "data", vec![attr]);
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_add_activity_alias(
    target_activity: String,
    alias_name: String,
    enabled: bool,
    label: Option<String>,
) {
    with_ctx(|ctx| {
        let mut warning = None;
        with_base_manifest_mut(ctx, |manifest| {
            let mut attrs = vec![
                manifest.make_string_attribute("name", 0x01010003, &alias_name),
                manifest.make_string_attribute("targetActivity", 0x01010202, &target_activity),
                manifest.make_bool_attribute("enabled", 0x01010000, enabled),
            ];
            if let Some(lbl) = label {
                attrs.push(manifest.make_string_attribute("label", 0x01010001, &lbl));
            }
            if let Some(app_idx) = manifest.find_element_index("application") {
                manifest.insert_child_element(app_idx, "activity-alias", attrs);
            } else {
                warning =
                    Some("manifest_add_activity_alias: application element not found".to_string());
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_add_activity_alias_in_component(
    component: String,
    target_activity: String,
    alias_name: String,
    enabled: bool,
    label: Option<String>,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        with_manifest_mut(ctx, index, |manifest| {
            let mut attrs = vec![
                manifest.make_string_attribute("name", 0x01010003, &alias_name),
                manifest.make_string_attribute("targetActivity", 0x01010202, &target_activity),
                manifest.make_bool_attribute("enabled", 0x01010000, enabled),
            ];
            if let Some(lbl) = label {
                attrs.push(manifest.make_string_attribute("label", 0x01010001, &lbl));
            }
            if let Some(app_idx) = manifest.find_element_index("application") {
                manifest.insert_child_element(app_idx, "activity-alias", attrs);
            } else {
                warning = Some(format!(
                    "manifest_add_activity_alias_in_component: application element not found in {component}"
                ));
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_copy_intent_filters(from_activity: String, to_activity: String) {
    with_ctx(|ctx| {
        let mut warning = None;
        with_base_manifest_mut(ctx, |manifest| {
            let from_idx = match manifest.find_element_with_attr(
                "activity",
                0x01010003,
                &from_activity,
            ) {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters: source activity '{from_activity}' not found"
                    ));
                    return;
                }
            };
            let to_idx = match manifest.find_element_with_attr("activity", 0x01010003, &to_activity)
            {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters: target activity '{to_activity}' not found"
                    ));
                    return;
                }
            };
            let from_end = match manifest.find_end_element(from_idx) {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters: failed to resolve end of '{from_activity}'"
                    ));
                    return;
                }
            };
            let mut filter_ranges = Vec::new();
            let mut i = from_idx + 1;
            while i < from_end {
                if let stitch_apk::axml::AxmlEvent::StartElement { name, .. } =
                    &manifest.elements[i]
                {
                    if manifest
                        .string(*name)
                        .map_or(false, |s| s == "intent-filter")
                    {
                        if let Some(end) = manifest.find_end_element(i) {
                            let events: Vec<_> = manifest.elements[i..=end].to_vec();
                            filter_ranges.push(events);
                            i = end + 1;
                            continue;
                        }
                    }
                }
                i += 1;
            }
            for events in filter_ranges.into_iter().rev() {
                let insert_pos = to_idx + 1;
                for (j, event) in events.into_iter().enumerate() {
                    manifest.elements.insert(insert_pos + j, event);
                }
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_copy_intent_filters_in_component(
    component: String,
    from_activity: String,
    to_activity: String,
) {
    with_ctx(|ctx| {
        let Some(index) = manifest_component_index(ctx, &component) else {
            return;
        };
        let mut warning = None;
        with_manifest_mut(ctx, index, |manifest| {
            let from_idx = match manifest.find_element_with_attr(
                "activity",
                0x01010003,
                &from_activity,
            ) {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters_in_component: source activity '{from_activity}' not found in {component}"
                    ));
                    return;
                }
            };
            let to_idx = match manifest.find_element_with_attr("activity", 0x01010003, &to_activity)
            {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters_in_component: target activity '{to_activity}' not found in {component}"
                    ));
                    return;
                }
            };
            let from_end = match manifest.find_end_element(from_idx) {
                Some(idx) => idx,
                None => {
                    warning = Some(format!(
                        "manifest_copy_intent_filters_in_component: failed to resolve end of '{from_activity}' in {component}"
                    ));
                    return;
                }
            };
            let mut filter_ranges = Vec::new();
            let mut i = from_idx + 1;
            while i < from_end {
                if let stitch_apk::axml::AxmlEvent::StartElement { name, .. } = &manifest.elements[i] {
                    if manifest.string(*name).map_or(false, |s| s == "intent-filter") {
                        if let Some(end) = manifest.find_end_element(i) {
                            let events: Vec<_> = manifest.elements[i..=end].to_vec();
                            filter_ranges.push(events);
                            i = end + 1;
                            continue;
                        }
                    }
                }
                i += 1;
            }
            for events in filter_ranges.into_iter().rev() {
                let insert_pos = to_idx + 1;
                for (j, event) in events.into_iter().enumerate() {
                    manifest.elements.insert(insert_pos + j, event);
                }
            }
        });
        if let Some(message) = warning {
            ctx.log().warn(message);
        }
    });
}

#[export]
pub fn manifest_get_document() -> u32 {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let slot_id = manifest_slot_id(0);
        if let Some((idx, _)) = docs
            .iter()
            .enumerate()
            .find(|(_, slot)| matches!(slot, Some((_, path)) if path == &slot_id))
        {
            return idx as u32;
        }
        drop(docs);
        with_ctx(|ctx| {
            let manifest = ctx.manifest().clone();
            XML_DOCUMENTS.with(|docs| {
                let mut docs = docs.borrow_mut();
                let handle = docs.len() as u32;
                docs.push(Some((manifest, slot_id)));
                handle
            })
        })
    })
}

#[export]
pub fn manifest_component_names() -> Vec<String> {
    with_ctx(|ctx| ctx.component_names())
}

#[export]
pub fn manifest_package_name_in_component(component: String) -> Option<String> {
    with_ctx(|ctx| {
        let index = manifest_component_index(ctx, &component)?;
        with_manifest(ctx, index, |manifest| {
            manifest.package_name().map(str::to_string)
        })
    })
}

#[export]
pub fn manifest_version_code_in_component(component: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = manifest_component_index(ctx, &component)?;
        with_manifest(ctx, index, |manifest| manifest.version_code())
    })
}

#[export]
pub fn manifest_version_name_in_component(component: String) -> Option<String> {
    with_ctx(|ctx| {
        let index = manifest_component_index(ctx, &component)?;
        with_manifest(ctx, index, |manifest| manifest.version_name().map(str::to_string))
    })
}

#[export]
pub fn manifest_min_sdk_version_in_component(component: String) -> Option<u32> {
    with_ctx(|ctx| {
        let index = manifest_component_index(ctx, &component)?;
        with_manifest(ctx, index, |manifest| manifest.min_sdk_version())
    })
}

#[export]
pub fn manifest_split_name_in_component(component: String) -> Option<String> {
    with_ctx(|ctx| {
        let index = manifest_component_index(ctx, &component)?;
        with_manifest(ctx, index, |manifest| manifest.split_name().map(str::to_string))
    })
}

#[export]
pub fn manifest_set_version_code_in_component(component: String, code: u32) {
    with_ctx(|ctx| {
        if let Some(index) = manifest_component_index(ctx, &component) {
            with_manifest_mut(ctx, index, |manifest| manifest.set_version_code(code));
        }
    });
}

#[export]
pub fn manifest_set_version_name_in_component(component: String, name: String) {
    with_ctx(|ctx| {
        if let Some(index) = manifest_component_index(ctx, &component) {
            with_manifest_mut(ctx, index, |manifest| manifest.set_version_name(&name));
        }
    });
}

#[export]
pub fn manifest_set_min_sdk_in_component(component: String, sdk: u32) {
    with_ctx(|ctx| {
        if let Some(index) = manifest_component_index(ctx, &component) {
            with_manifest_mut(ctx, index, |manifest| manifest.set_min_sdk(sdk));
        }
    });
}

#[export]
pub fn manifest_add_permission_in_component(component: String, permission: String) {
    with_ctx(|ctx| {
        if let Some(index) = manifest_component_index(ctx, &component) {
            with_manifest_mut(ctx, index, |manifest| manifest.add_permission(&permission));
        }
    });
}

#[export]
pub fn manifest_get_document_in_component(component: String) -> Option<u32> {
    with_ctx(|ctx| {
        let component_index = manifest_component_index(ctx, &component)?;
        let slot_id = manifest_slot_id(component_index);
        XML_DOCUMENTS.with(|docs| {
            let docs = docs.borrow();
            if let Some((idx, _)) = docs
                .iter()
                .enumerate()
                .find(|(_, slot)| matches!(slot, Some((_, path)) if path == &slot_id))
            {
                return Some(idx as u32);
            }
            drop(docs);
            let manifest = ctx.component_manifest(component_index)?.clone();
            XML_DOCUMENTS.with(|docs| {
                let mut docs = docs.borrow_mut();
                let handle = docs.len() as u32;
                docs.push(Some((manifest, slot_id)));
                Some(handle)
            })
        })
    })
}

fn parse_config_changes(s: &str) -> (i32, Vec<String>) {
    let mut flags = 0i32;
    let mut warnings = Vec::new();
    for part in s.split('|') {
        flags |= match part.trim() {
            "mcc" => 0x0001,
            "mnc" => 0x0002,
            "locale" => 0x0004,
            "touchscreen" => 0x0008,
            "keyboard" => 0x0010,
            "keyboardHidden" => 0x0020,
            "navigation" => 0x0040,
            "orientation" => 0x0080,
            "screenLayout" => 0x0100,
            "uiMode" => 0x0200,
            "screenSize" => 0x0400,
            "smallestScreenSize" => 0x0800,
            "density" => 0x1000,
            "layoutDirection" => 0x2000,
            "colorMode" => 0x4000,
            "fontScale" => 0x40000000,
            other => {
                warnings.push(format!(
                    "manifest_set_activity_config_changes: unknown configChanges flag '{other}'"
                ));
                0
            }
        };
    }
    (flags, warnings)
}
