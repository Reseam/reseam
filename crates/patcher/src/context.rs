// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::reseam_dex::{ClassSkeleton, EncodedMethod};
use reseam_apk::ApkFile;

use crate::log::{LogEntry, PatchLog};
use crate::options::PatchOptions;

mod components;
mod dex;
mod files;
mod resources;

#[derive(Debug, Clone, Copy)]
pub struct InstructionLocation {
    pub dex_idx: usize,
    pub class_idx: usize,
    pub method_pos: usize,
    pub is_virtual: bool,
    pub insn_idx: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MethodCallSiteHit {
    pub loc: InstructionLocation,
    pub target_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldAccessSiteHit {
    pub loc: InstructionLocation,
    pub target_index: usize,
}

#[derive(Debug, Clone, Copy)]
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
    pub(crate) skeleton: Option<CachedSkeleton>,
    /// The method most recently decoded for inspection: patches read a
    /// method one instruction per FFI call, and this decodes it once.
    pub(crate) method: Option<CachedMethod>,
}

pub(crate) struct CachedSkeleton {
    pub(crate) dex_idx: usize,
    pub(crate) class_idx: usize,
    pub(crate) skeleton: ClassSkeleton,
}

pub(crate) struct CachedMethod {
    pub(crate) location: MethodLocation,
    pub(crate) method: EncodedMethod,
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

    pub fn log(&mut self) -> &mut PatchLog {
        &mut self.log
    }

    pub fn set_log(&mut self, log: PatchLog) {
        self.log = log;
    }

    pub fn take_log_entries(&mut self) -> Vec<LogEntry> {
        self.log.take_entries()
    }

    pub fn options(&self) -> &PatchOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: PatchOptions) {
        self.options = options;
    }

    pub fn clear_options(&mut self) {
        self.options = PatchOptions::default();
    }

    pub fn package_name(&self) -> Option<&str> {
        self.apk.package_name()
    }

    pub fn version_code(&self) -> Option<u32> {
        self.apk.version_code()
    }

    pub fn version_name(&self) -> Option<&str> {
        self.apk.version_name()
    }

    pub fn apk(&self) -> &ApkFile {
        self.apk
    }

    pub fn apk_mut(&mut self) -> &mut ApkFile {
        self.apk
    }
}
