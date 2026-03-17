#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub patch: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct PatchLog {
    entries: Vec<LogEntry>,
    patch_name: String,
}

impl PatchLog {
    pub fn new(patch_name: String) -> Self {
        Self {
            entries: Vec::new(),
            patch_name,
        }
    }

    pub fn debug(&mut self, message: impl Into<String>) {
        self.entries.push(LogEntry {
            level: LogLevel::Debug,
            patch: self.patch_name.clone(),
            message: message.into(),
        });
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.entries.push(LogEntry {
            level: LogLevel::Info,
            patch: self.patch_name.clone(),
            message: message.into(),
        });
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.entries.push(LogEntry {
            level: LogLevel::Warn,
            patch: self.patch_name.clone(),
            message: message.into(),
        });
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn take_entries(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.entries)
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
        }
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.level, self.patch, self.message)
    }
}
