// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a patch sees while it runs: the APK session plus the run's log,
//! options, and decode caches.

mod dex;
mod extensions;
mod files;

pub use extensions::ExtensionSet;

use reseam_apk::reseam_dex::{ClassSkeleton, CodeItem, DexFile, EncodedMethod};
use reseam_apk::ApkFile;

use crate::log::{LogEntry, PatchLog};
use crate::options::PatchOptions;

/// A method named by class descriptor, name, and prototype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodKey<'a> {
    pub class: &'a str,
    pub name: &'a str,
    pub proto: &'a str,
}

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
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct InstructionLocation {
    pub method: MethodLocation,
    pub insn_idx: usize,
}

/// An instruction referring to one of the targets a search asked for.
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
    /// Skeleton of the deferred class most recently inspected: patches walk a
    /// class's methods one FFI call at a time, and this keeps that linear.
    skeleton: Option<CachedSkeleton>,
    /// The method most recently decoded for inspection: patches read a
    /// method one instruction per FFI call, and this decodes it once.
    method: Option<CachedMethod>,
    extensions: ExtensionSet,
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

/// The encoded method at `m` in a DEX whose class is materialized.
pub fn method_mut(dex: &mut DexFile, m: MethodLocation) -> Option<&mut EncodedMethod> {
    let data = dex.class_mut(m.class_idx).ok()?.class_data.as_mut()?;
    let list = if m.is_virtual {
        &mut data.virtual_methods
    } else {
        &mut data.direct_methods
    };
    list.get_mut(m.method_idx)
}

pub fn code_mut(dex: &mut DexFile, m: MethodLocation) -> Option<&mut CodeItem> {
    method_mut(dex, m)?.code.as_mut()
}
