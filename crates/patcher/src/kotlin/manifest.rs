use boltffi::export;

use super::{with_ctx, XML_DOCUMENTS};

#[export]
pub fn manifest_package_name() -> String {
    with_ctx(|ctx| {
        ctx.manifest()
            .package_name()
            .unwrap_or_default()
            .to_string()
    })
}

#[export]
pub fn manifest_version_code() -> Option<u32> {
    with_ctx(|ctx| ctx.manifest().version_code())
}

#[export]
pub fn manifest_version_name() -> Option<String> {
    with_ctx(|ctx| ctx.manifest().version_name().map(|s| s.to_string()))
}

#[export]
pub fn manifest_min_sdk_version() -> Option<u32> {
    with_ctx(|ctx| ctx.manifest().min_sdk_version())
}

#[export]
pub fn manifest_split_name() -> Option<String> {
    with_ctx(|ctx| ctx.manifest().split_name().map(|s| s.to_string()))
}

#[export]
pub fn manifest_set_version_code(code: u32) {
    with_ctx(|ctx| ctx.manifest_mut().set_version_code(code));
}

#[export]
pub fn manifest_set_version_name(name: String) {
    with_ctx(|ctx| ctx.manifest_mut().set_version_name(&name));
}

#[export]
pub fn manifest_set_min_sdk(sdk: u32) {
    with_ctx(|ctx| ctx.manifest_mut().set_min_sdk(sdk));
}

#[export]
pub fn manifest_add_permission(permission: String) {
    with_ctx(|ctx| ctx.manifest_mut().add_permission(&permission));
}

#[export]
pub fn manifest_set_attribute_int(element_name: String, attr_res_id: u32, value: i32) {
    with_ctx(|ctx| {
        if let Some(idx) = ctx.manifest().find_element_index(&element_name) {
            ctx.manifest_mut()
                .set_element_attribute_int(idx, attr_res_id, value);
        }
    });
}

#[export]
pub fn manifest_set_attribute_string(element_name: String, attr_res_id: u32, value: String) {
    with_ctx(|ctx| {
        if let Some(idx) = ctx.manifest().find_element_index(&element_name) {
            ctx.manifest_mut()
                .add_element_attribute_string(idx, &element_name, attr_res_id, &value);
        }
    });
}

#[export]
pub fn manifest_set_activity_config_changes(activity_name: String, config_changes: String) {
    with_ctx(|ctx| {
        if let Some(idx) =
            ctx.manifest()
                .find_element_with_attr("activity", 0x01010003, &activity_name)
        {
            let flags = parse_config_changes(&config_changes);
            ctx.manifest_mut()
                .set_element_attribute_int(idx, 0x0101001f, flags);
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
        let act_idx = match ctx
            .manifest()
            .find_element_with_attr("activity", 0x01010003, &activity_name)
        {
            Some(idx) => idx,
            None => return,
        };
        let manifest = ctx.manifest_mut();
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
}

#[export]
pub fn manifest_add_activity_alias(
    target_activity: String,
    alias_name: String,
    enabled: bool,
    label: Option<String>,
) {
    with_ctx(|ctx| {
        let manifest = ctx.manifest_mut();
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
        }
    });
}

#[export]
pub fn manifest_copy_intent_filters(from_activity: String, to_activity: String) {
    with_ctx(|ctx| {
        let manifest = ctx.manifest();
        let from_idx = match manifest.find_element_with_attr("activity", 0x01010003, &from_activity)
        {
            Some(idx) => idx,
            None => return,
        };
        let to_idx = match manifest.find_element_with_attr("activity", 0x01010003, &to_activity) {
            Some(idx) => idx,
            None => return,
        };
        let from_end = match manifest.find_end_element(from_idx) {
            Some(idx) => idx,
            None => return,
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
        let manifest = ctx.manifest_mut();
        for events in filter_ranges.into_iter().rev() {
            let insert_pos = to_idx + 1;
            for (j, event) in events.into_iter().enumerate() {
                manifest.elements.insert(insert_pos + j, event);
            }
        }
    });
}

#[export]
pub fn manifest_get_document() -> u32 {
    with_ctx(|ctx| {
        let manifest = ctx.manifest().clone();
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            let handle = docs.len() as u32;
            docs.push(Some((manifest, "AndroidManifest.xml".to_string())));
            handle
        })
    })
}

fn parse_config_changes(s: &str) -> i32 {
    let mut flags = 0i32;
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
            _ => 0,
        };
    }
    flags
}
