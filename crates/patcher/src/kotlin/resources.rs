use boltffi::export;

use super::types::ResourceRef;
use super::{with_ctx, BUNDLE_DIR};

#[export]
pub fn res_has_resources() -> bool {
    with_ctx(|ctx| ctx.resources().is_some())
}

#[export]
pub fn res_get_string(index: u32) -> Option<String> {
    with_ctx(|ctx| {
        ctx.resources()
            .and_then(|r| r.get_string(index).map(|s| s.to_string()))
    })
}

#[export]
pub fn res_set_string(index: u32, value: String) {
    with_ctx(|ctx| {
        if let Some(res) = ctx.resources_mut() {
            res.set_string(index, value);
        }
    });
}

#[export]
pub fn res_resource_id(res_type: String, res_name: String) -> Option<i64> {
    with_ctx(|ctx| ctx.find_resource_id(&res_type, &res_name).map(|id| id as i64))
}

#[export]
pub fn res_find_entries_by_string(string_index: u32) -> Vec<ResourceRef> {
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
                type_id: e.type_id as u8,
                entry_index: e.entry_index as u16,
                key_name: e.key_name,
            })
            .collect()
    })
}

#[export]
pub fn res_add_string_resource(name: String, value: String) -> Option<u32> {
    with_ctx(|ctx| ctx.resources_mut()?.add_string_resource(&name, &value))
}

#[export]
pub fn res_replace_entry_string(res_id: u32, new_string_index: u32) {
    with_ctx(|ctx| {
        if let Some(res) = ctx.resources_mut() {
            res.replace_entry_string(res_id, new_string_index);
        }
    });
}

#[export]
pub fn res_copy_file(bundle_path: String, apk_path: String) {
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
            ctx.log()
                .warn(format!("copy_file: failed to read {}: {e}", full_path.display()))
        }),
    }
}

#[export]
pub fn res_copy_resource_group(res_type: String, files: Vec<String>) {
    let bundle_dir = BUNDLE_DIR.with(|bd| bd.borrow().clone());
    let bundle_dir = match bundle_dir {
        Some(dir) => dir,
        None => return,
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
                    "copy_resource_group: failed to read {}: {e}",
                    src.display()
                ))
            }),
        }
    }
}

#[export]
pub fn res_delete_file(apk_path: String) {
    with_ctx(|ctx| ctx.delete_file(&apk_path));
}

#[export]
pub fn res_list_files(prefix: String) -> Vec<String> {
    with_ctx(|ctx| {
        ctx.list_files()
            .iter()
            .filter(|f| f.starts_with(&prefix))
            .cloned()
            .collect()
    })
}
