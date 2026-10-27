// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::OsStr;
use std::path::Path;

use boltffi::export;
use reseam_apk::Compression;
use reseam_apk::axml::{AxmlDocument, is_compiled_axml};

use super::components::{component, with_component};
use super::xml::attributes::find_attribute;
use crate::bridge::phase::bundle_path;
use crate::bridge::run_state::{checked, record_failure};
use crate::context::PatchContext;

/// The largest range [`file_source_range`] reads at once.
const SOURCE_RANGE_LIMIT: u32 = 1024 * 1024;

#[export]
pub fn file_list(component_name: Option<String>) -> Vec<String> {
    checked(with_component(component_name, |ctx, index| {
        Ok(component(ctx, index)
            .entry_names()
            .iter()
            .map(ToString::to_string)
            .collect())
    }))
}

#[export]
pub fn file_read(component_name: Option<String>, apk_path: String) -> Option<Vec<u8>> {
    checked(with_component(component_name, |ctx, index| {
        ctx.read_file(index, &apk_path)
            .map_err(|error| error.to_string())
    }))
}

#[export]
pub fn file_xml_with(component_name: Option<String>, attributes: Vec<String>) -> Vec<String> {
    let names: Vec<&str> = attributes.iter().map(String::as_str).collect();
    checked(with_component(component_name, |ctx, index| {
        let paths: Vec<String> = component(ctx, index)
            .entry_names()
            .iter()
            .filter(|path| Path::new(path.as_str()).extension() == Some(OsStr::new("xml")))
            .map(ToString::to_string)
            .collect();
        paths
            .into_iter()
            .filter_map(|path| {
                carries_attribute(ctx, index, &path, &names)
                    .map(|found| found.then_some(path))
                    .transpose()
            })
            .collect()
    }))
}

fn carries_attribute(
    ctx: &mut PatchContext<'_>,
    component: usize,
    path: &str,
    names: &[&str],
) -> Result<bool, String> {
    let in_file = |error: &dyn std::fmt::Display| format!("{path}: {error}");
    let Some(bytes) = ctx
        .read_file(component, path)
        .map_err(|error| in_file(&error))?
    else {
        return Ok(false);
    };
    if !is_compiled_axml(&bytes)
        || !AxmlDocument::may_carry_attribute(&bytes, names).map_err(|error| in_file(&error))?
    {
        return Ok(false);
    }
    let document = AxmlDocument::parse(&bytes).map_err(|error| in_file(&error))?;
    Ok((0..document.events().len()).any(|index| {
        names
            .iter()
            .any(|name| find_attribute(&document, document.attributes(index), name).is_some())
    }))
}

#[export]
pub fn file_source_size(component_name: Option<String>) -> Result<u64, String> {
    with_component(component_name, |ctx, index| {
        component(ctx, index)
            .source()
            .map(|source| source.len() as u64)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn file_source_range(
    component_name: Option<String>,
    offset: u64,
    length: u32,
) -> Result<Vec<u8>, String> {
    if length > SOURCE_RANGE_LIMIT {
        return Err("APK source reads must not exceed one MiB".to_string());
    }
    with_component(component_name, |ctx, index| {
        let source = component(ctx, index)
            .source()
            .map_err(|error| error.to_string())?;
        let offset = usize::try_from(offset)
            .map_err(|_| "APK source offset exceeds address space".to_string())?;
        let remaining = source
            .get(offset..)
            .ok_or_else(|| "APK source offset exceeds file size".to_string())?;
        Ok(remaining[..remaining.len().min(length as usize)].to_vec())
    })
}

#[export]
pub fn file_signers(component_name: Option<String>) -> Vec<Vec<u8>> {
    checked(with_component(component_name, |ctx, index| {
        let source = component(ctx, index)
            .source()
            .map_err(|error| error.to_string())?;
        reseam_sign::signer_certificates(&source).map_err(|error| format!("file_signers: {error}"))
    }))
}

#[export]
pub fn file_inject(
    component_name: Option<String>,
    apk_path: String,
    data: Vec<u8>,
    compression: FileCompression,
) {
    inject(component_name, &apk_path, data, compression.into());
}

#[export]
pub fn file_delete(component_name: Option<String>, apk_path: String) {
    checked(with_component(component_name, |ctx, index| {
        ctx.delete_file(index, &apk_path)
            .map_err(|error| format!("file_delete {apk_path}: {error}"))
    }));
}

#[export]
pub fn file_copy(component_name: Option<String>, bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(component_name, &apk_path, data, Compression::Deflated),
        Err(error) => record_failure(format!("read bundle file {}: {error}", source.display())),
    }
}

fn inject(component_name: Option<String>, apk_path: &str, data: Vec<u8>, compression: Compression) {
    checked(with_component(component_name, |ctx, index| {
        ctx.inject_file(index, apk_path, data, compression)
            .map_err(|error| format!("inject {apk_path}: {error}"))
    }));
}

/// How an injected file is stored in the APK.
#[boltffi::data]
#[derive(Debug, Clone, Copy)]
pub enum FileCompression {
    Deflated,
    /// Uncompressed, for entries Android maps from the APK directly.
    Stored,
}

impl From<FileCompression> for Compression {
    fn from(compression: FileCompression) -> Self {
        match compression {
            FileCompression::Deflated => Compression::Deflated,
            FileCompression::Stored => Compression::Stored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::phase::{ContextGuard, RunningBundle};
    use crate::bridge::run_state::RunGuard;
    use crate::context::{ExtensionSet, PatchContext};

    #[test]
    fn original_apk_ranges_are_bounded_and_report_invalid_requests() {
        let (directory, mut apk) = crate::test_support::apk();
        let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
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
