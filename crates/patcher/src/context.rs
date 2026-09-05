// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a patch sees while it runs: the APK session plus the run's log,
//! options, and decode caches.

mod dex;
mod files;

use reseam_apk::reseam_dex::{ClassSkeleton, EncodedMethod};
use reseam_apk::ApkFile;

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
