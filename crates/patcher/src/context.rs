// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod app_entry;
mod dex;
mod extensions;
mod files;
mod hermes;
mod search;

pub use dex::ClassFields;
pub use extensions::ExtensionSet;
pub use search::MethodRefQuery;

use reseam_apk::ApkFile;
use reseam_apk::reseam_dex::{ClassSkeleton, CodeItem, DexFile, EncodedMethod, MethodKind};

use crate::log::{LogEntry, PatchLog};
use crate::options::PatchOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassLocation {
    pub dex_idx: usize,
    pub class_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodLocation {
    pub dex_idx: usize,
    pub class_idx: usize,
    pub method_idx: usize,
    pub kind: MethodKind,
}

#[derive(Debug, Clone, Copy)]
pub struct InstructionLocation {
    pub method: MethodLocation,
    pub insn_idx: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SiteHit {
    pub loc: InstructionLocation,
    pub target_index: usize,
}

#[derive(Debug, Clone)]
pub struct FingerprintLocation {
    pub method: MethodLocation,
    pub matched_indices: Vec<u32>,
}

pub struct PatchContext<'a> {
    apk: &'a mut ApkFile,
    log: PatchLog,
    options: PatchOptions,
    skeleton: Option<CachedSkeleton>,
    method: Option<CachedMethod>,
    extensions: ExtensionSet,
    hermes: Option<hermes::HermesSession>,
}

struct CachedSkeleton {
    location: ClassLocation,
    skeleton: ClassSkeleton,
}

struct CachedMethod {
    location: MethodLocation,
    method: EncodedMethod,
}

impl<'a> PatchContext<'a> {
    pub fn new(apk: &'a mut ApkFile) -> Self {
        Self {
            apk,
            log: PatchLog::default(),
            options: PatchOptions::default(),
            skeleton: None,
            method: None,
            extensions: ExtensionSet::default(),
            hermes: None,
        }
    }

    pub fn apk(&self) -> &ApkFile {
        self.apk
    }

    pub fn apk_mut(&mut self) -> &mut ApkFile {
        self.method = None;
        self.skeleton = None;
        self.apk
    }

    pub fn log(&mut self) -> &mut PatchLog {
        &mut self.log
    }

    pub fn options(&self) -> &PatchOptions {
        &self.options
    }

    pub(crate) fn begin_patch(&mut self, patch: &str, options: PatchOptions) {
        self.log = PatchLog::new(patch);
        self.options = options;
    }

    pub(crate) fn take_log_entries(&mut self) -> Vec<LogEntry> {
        self.log.take_entries()
    }
}

pub fn method_mut(
    dex: &mut DexFile,
    m: MethodLocation,
) -> crate::error::Result<Option<&mut EncodedMethod>> {
    let Some(data) = dex.class_mut(m.class_idx)?.class_data.as_mut() else {
        return Ok(None);
    };
    let list = if m.kind == MethodKind::Virtual {
        &mut data.virtual_methods
    } else {
        &mut data.direct_methods
    };
    Ok(list.get_mut(m.method_idx))
}

pub fn code_mut(
    dex: &mut DexFile,
    m: MethodLocation,
) -> crate::error::Result<Option<&mut CodeItem>> {
    Ok(method_mut(dex, m)?.and_then(|method| method.code.as_mut()))
}
