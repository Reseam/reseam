// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub use reseam_model::{LogEntry, LogLevel};

use crate::options::PatchOptions;
use crate::patch::PatchRef;

/// The log and options of the patch a context runs for.
#[derive(Default)]
pub struct PatchScope {
    pub log: PatchLog,
    pub options: PatchOptions,
}

impl PatchScope {
    pub(crate) fn new(patch: &PatchRef, options: PatchOptions) -> Self {
        Self {
            log: PatchLog::new(patch.clone()),
            options,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PatchLog {
    patch: PatchRef,
    entries: Vec<LogEntry>,
}

impl PatchLog {
    pub fn new(patch: PatchRef) -> Self {
        Self {
            patch,
            entries: Vec::new(),
        }
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        self.entries.push(LogEntry {
            level,
            patch: self.patch.clone(),
            message: message.into(),
        });
    }

    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    pub(crate) fn take_entries(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.entries)
    }
}
