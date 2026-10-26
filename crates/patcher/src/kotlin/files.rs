// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::OsStr;
use std::path::Path;

use boltffi::export;
use reseam_apk::Compression;
use reseam_apk::axml::{AxmlDocument, AxmlEvent, is_compiled_axml};

use super::handles::{bundle_path, checked, record_failure, with_ctx};
use super::xml_attributes::find_attribute;
use crate::context::PatchContext;

pub(super) fn component_index(ctx: &PatchContext<'_>, component: Option<&str>) -> Option<usize> {
    match component {
        None => Some(0),
        Some(name) => ctx.apk().component_by_name(name),
    }
}

pub(super) fn with_component<R>(
    component: Option<String>,
    f: impl FnOnce(&mut PatchContext<'_>, usize) -> R,
) -> Option<R> {
    with_ctx(|ctx| {
        if let Some(index) = component_index(ctx, component.as_deref()) {
            Some(f(ctx, index))
        } else {
            record_failure(format!("unknown component {component:?}"));
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
            .expect("component checked by with_component")
            .entry_names()
            .iter()
            .map(ToString::to_string)
            .collect()
    })
    .unwrap_or_default()
}

#[export]
pub fn file_read(component: Option<String>, apk_path: String) -> Option<Vec<u8>> {
    with_component(component, |ctx, index| {
        checked(ctx.read_file(index, &apk_path))
    })
    .flatten()
}

#[export]
pub fn file_xml_with(component: Option<String>, attributes: Vec<String>) -> Vec<String> {
    let names: Vec<&str> = attributes.iter().map(String::as_str).collect();
    with_component(component, |ctx, index| {
        let paths: Vec<String> = ctx
            .apk()
            .component(index)
            .expect("component checked by with_component")
            .entry_names()
            .iter()
            .filter(|path| Path::new(path.as_str()).extension() == Some(OsStr::new("xml")))
            .map(ToString::to_string)
            .collect();
        checked(
            paths
                .into_iter()
                .filter_map(|path| {
                    carries_attribute(ctx, index, &path, &names)
                        .map(|found| found.then_some(path))
                        .transpose()
                })
                .collect::<Result<Vec<_>, String>>(),
        )
    })
    .unwrap_or_default()
}

fn carries_attribute(
    ctx: &mut PatchContext<'_>,
    component: usize,
    path: &str,
    names: &[&str],
) -> Result<bool, String> {
    let in_file = |error: reseam_apk::ApkError| format!("{path}: {error}");
    let Some(bytes) = ctx
        .read_file(component, path)
        .map_err(|error| error.to_string())?
    else {
        return Ok(false);
    };
    if !is_compiled_axml(&bytes)
        || !AxmlDocument::may_carry_attribute(&bytes, names).map_err(in_file)?
    {
        return Ok(false);
    }
    let document = AxmlDocument::parse(&bytes).map_err(in_file)?;
    Ok(document.events().iter().any(|event| {
        matches!(event, AxmlEvent::StartElement { attributes, .. }
            if names.iter().any(|name| find_attribute(&document, attributes.as_slice(), name).is_some()))
    }))
}

#[export]
pub fn file_source_size(component: Option<String>) -> Result<u64, String> {
    with_component(component, |ctx, index| {
        let component = ctx
            .apk()
            .component(index)
            .expect("component checked by with_component");
        component
            .source()
            .map(|source| source.len() as u64)
            .map_err(|error| error.to_string())
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn file_source_range(
    component: Option<String>,
    offset: u64,
    length: u32,
) -> Result<Vec<u8>, String> {
    if length > 1024 * 1024 {
        return Err("APK source reads must not exceed one MiB".to_string());
    }
    with_component(component, |ctx, index| {
        let component = ctx
            .apk()
            .component(index)
            .expect("component checked by with_component");
        let source = component.source().map_err(|error| error.to_string())?;
        let offset = usize::try_from(offset)
            .map_err(|_| "APK source offset exceeds address space".to_string())?;
        let remaining = source
            .get(offset..)
            .ok_or_else(|| "APK source offset exceeds file size".to_string())?;
        Ok(remaining[..remaining.len().min(length as usize)].to_vec())
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn file_signers(component: Option<String>) -> Vec<Vec<u8>> {
    with_component(component, |ctx, index| {
        let certificates = ctx
            .apk()
            .component(index)
            .expect("component checked by with_component")
            .source()
            .map_err(|error| error.to_string())
            .and_then(|source| {
                reseam_sign::signer_certificates(&source).map_err(|error| error.to_string())
            });
        match certificates {
            Ok(certificates) => certificates,
            Err(error) => {
                record_failure(&error);
                ctx.log().warn(format!("file_signers: {error}"));
                Vec::new()
            }
        }
    })
    .unwrap_or_default()
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
            record_failure(&error);
            ctx.log().warn(format!("file_delete {apk_path}: {error}"));
        }
    });
}

#[export]
pub fn file_copy(component: Option<String>, bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(component, &apk_path, data, Compression::Deflated),
        Err(error) => with_ctx(|ctx| {
            record_failure(format!("read bundle file {}: {error}", source.display()));
            ctx.log().warn(format!(
                "file_copy: failed to read {}: {error}",
                source.display()
            ));
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
            record_failure(&error);
            ctx.log().warn(format!("inject {apk_path}: {error}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotlin::handles::{ContextGuard, RunGuard, RunningBundle};

    #[test]
    fn original_apk_ranges_are_bounded_and_report_invalid_requests() {
        let (directory, mut apk) = crate::test_support::apk();
        let mut context = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        let size = file_source_size(None).unwrap();
        assert_eq!(file_source_range(None, 0, 4).unwrap(), b"PK\x03\x04");
        for (offset, length) in [(size, 0), (size, 100), (size - 1, 0)] {
            assert!(file_source_range(None, offset, length).unwrap().is_empty());
        }
        assert_eq!(file_source_range(None, size - 1, 100).unwrap().len(), 1);
        for (offset, length) in [(size + 1, 1), (u64::MAX, 1), (0, 1024 * 1024 + 1)] {
            assert!(file_source_range(None, offset, length).is_err());
        }
        guard.finish().unwrap();
    }
}
