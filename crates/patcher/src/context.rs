// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod app_entry;
mod dex;
mod extensions;
mod files;
mod hermes;
mod hierarchy;
mod search;
mod undo;

pub use dex::ClassFields;
pub use extensions::ExtensionSet;
pub(crate) use hermes::hermes_imports;
pub use search::{FieldRefQuery, MethodRefQuery};

use reseam_apk::ApkFile;
use reseam_dex::{ClassSkeleton, CodeItem, DexFile, EncodedMethod, MethodKind};

use crate::log::{PatchLog, PatchScope};
use crate::options::PatchOptions;
use crate::patch::PatchRef;

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

pub struct PatchContext<'a> {
    apk: &'a mut ApkFile,
    scope: PatchScope,
    skeleton: Option<CachedSkeleton>,
    method: Option<CachedMethod>,
    extensions: ExtensionSet,
    hermes: Option<hermes::HermesSession>,
    journal: reseam_journal::Journal<undo::Part, undo::Undo>,
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
    /// A context over `apk` that links classes from `extensions`.
    pub fn new(apk: &'a mut ApkFile, extensions: ExtensionSet) -> Self {
        Self {
            apk,
            scope: PatchScope::default(),
            skeleton: None,
            method: None,
            extensions,
            hermes: None,
            journal: reseam_journal::Journal::default(),
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
        &mut self.scope.log
    }

    pub fn options(&self) -> &PatchOptions {
        &self.scope.options
    }

    pub(crate) fn scope(&mut self) -> &mut PatchScope {
        &mut self.scope
    }

    pub(crate) fn begin_patch(&mut self, patch: &PatchRef, options: PatchOptions) {
        self.scope = PatchScope::new(patch, options);
    }
}

pub fn method_mut(
    dex: &mut DexFile,
    m: MethodLocation,
) -> crate::error::Result<Option<&mut EncodedMethod>> {
    let Some(data) = dex.class_mut(m.class_idx)?.class_data.as_mut() else {
        return Ok(None);
    };
    Ok(data.methods_mut(m.kind).get_mut(m.method_idx))
}

pub fn code_mut(
    dex: &mut DexFile,
    m: MethodLocation,
) -> crate::error::Result<Option<&mut CodeItem>> {
    Ok(method_mut(dex, m)?.and_then(EncodedMethod::code_mut))
}
