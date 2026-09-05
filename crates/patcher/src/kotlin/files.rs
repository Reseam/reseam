// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Loose APK entries. `component` is a split name; `None` means the base.

use boltffi::export;
use reseam_apk::Compression;

use super::handles::{bundle_path, with_ctx};
use crate::context::PatchContext;

/// The component index a patch named, defaulting to the base.
pub(super) fn component_index(ctx: &PatchContext<'_>, component: Option<&str>) -> Option<usize> {
    match component {
        None => Some(0),
        Some(name) => ctx.apk().component_by_name(name),
    }
}

/// Runs `f` on the named component, logging a warning when the name is unknown.
pub(super) fn with_component<R>(
    component: Option<String>,
    f: impl FnOnce(&mut PatchContext<'_>, usize) -> R,
) -> Option<R> {
    with_ctx(|ctx| match component_index(ctx, component.as_deref()) {
        Some(index) => Some(f(ctx, index)),
        None => {
            ctx.log().warn(format!(
                "unknown component {}",
                component.unwrap_or_default()
            ));
            None
        }
    })
}

#[export]
pub fn component_names() -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .components()
            .iter()
            .map(|c| c.name().to_string())
            .collect()
    })
}

#[export]
pub fn file_list(component: Option<String>) -> Vec<String> {
    with_component(component, |ctx, index| {
        ctx.apk()
            .component(index)
            .map(|c| c.entry_names().iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[export]
pub fn file_read(component: Option<String>, apk_path: String) -> Option<Vec<u8>> {
    with_component(component, |ctx, index| {
        ctx.read_file(index, &apk_path).ok().flatten()
    })
    .flatten()
}

#[export]
pub fn file_inject(component: Option<String>, apk_path: String, data: Vec<u8>, stored: bool) {
    let compression = if stored {
        Compression::Stored
    } else {
        Compression::Deflated
    };
    inject(component, &apk_path, data, compression);
}

#[export]
pub fn file_delete(component: Option<String>, apk_path: String) {
    with_component(component, |ctx, index| {
        if let Err(error) = ctx.delete_file(index, &apk_path) {
            ctx.log().warn(format!("file_delete {apk_path}: {error}"));
        }
    });
}

/// Copies a file from the bundle into the APK.
#[export]
pub fn file_copy(component: Option<String>, bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(component, &apk_path, data, Compression::Deflated),
        Err(error) => with_ctx(|ctx| {
            ctx.log().warn(format!(
                "file_copy: failed to read {}: {error}",
                source.display()
            ))
        }),
    }
}

pub(super) fn inject(
    component: Option<String>,
    apk_path: &str,
    data: Vec<u8>,
    compression: Compression,
) {
    with_component(component, |ctx, index| {
        if let Err(error) = ctx.inject_file(index, apk_path, data, compression) {
            ctx.log().warn(format!("inject {apk_path}: {error}"));
        }
    });
}
