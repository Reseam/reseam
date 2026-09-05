// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: LogLevel,
    pub patch: String,
    pub message: String,
}

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

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
        })
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.level, self.patch, self.message)
    }
}
