// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub use reseam_model::{LogEntry, LogLevel};

/// Messages a patch emits while it runs, tagged with the patch's name.
#[derive(Debug, Clone, Default)]
pub struct PatchLog {
    patch: String,
    entries: Vec<LogEntry>,
}

impl PatchLog {
    pub fn new(patch: impl Into<String>) -> Self {
        Self {
            patch: patch.into(),
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
