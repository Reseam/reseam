// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use boltffi::export;

use super::{with_ctx, BUNDLE_DIR};

fn component_index(ctx: &crate::context::PatchContext<'_>, component: &str) -> Option<usize> {
    ctx.component_index(component)
}

#[export]
pub fn file_component_names() -> Vec<String> {
    with_ctx(|ctx| ctx.component_names())
}

#[export]
pub fn file_list() -> Vec<String> {
    with_ctx(|ctx| {
        ctx.list_files_in_component(0)
            .unwrap_or(&[])
            .iter()
            .map(ToString::to_string)
            .collect()
    })
}

#[export]
pub fn file_list_in_component(component: String) -> Vec<String> {
    with_ctx(|ctx| {
        let Some(index) = component_index(ctx, &component) else {
            return Vec::new();
        };
        ctx.list_files_in_component(index)
            .unwrap_or(&[])
            .iter()
            .map(ToString::to_string)
            .collect()
    })
}

#[export]
pub fn file_read(apk_path: String) -> Option<Vec<u8>> {
    with_ctx(|ctx| ctx.read_file_from_component(0, &apk_path))
}

#[export]
pub fn file_read_in_component(component: String, apk_path: String) -> Option<Vec<u8>> {
    with_ctx(|ctx| {
        let index = component_index(ctx, &component)?;
        ctx.read_file_from_component(index, &apk_path)
    })
}

#[export]
pub fn file_inject(apk_path: String, data: Vec<u8>) {
    with_ctx(|ctx| ctx.inject_file(&apk_path, data));
}

#[export]
pub fn file_inject_in_component(component: String, apk_path: String, data: Vec<u8>) {
    with_ctx(|ctx| {
        if let Some(index) = component_index(ctx, &component) {
            ctx.inject_file_into_component(index, &apk_path, data);
        }
    });
}

#[export]
pub fn file_delete(apk_path: String) {
    with_ctx(|ctx| ctx.delete_file(&apk_path));
}

#[export]
pub fn file_delete_in_component(component: String, apk_path: String) {
    with_ctx(|ctx| {
        if let Some(index) = component_index(ctx, &component) {
            ctx.delete_file_from_component(index, &apk_path);
        }
    });
}

#[export]
pub fn file_copy(bundle_path: String, apk_path: String) {
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
                "file_copy: failed to read {}: {e}",
                full_path.display()
            ))
        }),
    }
}

#[export]
pub fn file_copy_in_component(component: String, bundle_path: String, apk_path: String) {
    let full_path = BUNDLE_DIR.with(|bd| {
        let bd = bd.borrow();
        match bd.as_ref() {
            Some(dir) => dir.join(&bundle_path),
            None => std::path::PathBuf::from(&bundle_path),
        }
    });
    match std::fs::read(&full_path) {
        Ok(data) => with_ctx(|ctx| {
            if let Some(index) = component_index(ctx, &component) {
                ctx.inject_file_into_component(index, &apk_path, data);
            }
        }),
        Err(e) => with_ctx(|ctx| {
            ctx.log().warn(format!(
                "file_copy_in_component: failed to read {}: {e}",
                full_path.display()
            ))
        }),
    }
}
