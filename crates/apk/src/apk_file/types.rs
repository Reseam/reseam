// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Borrow;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentName(Box<str>);

impl ComponentName {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ComponentName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for ComponentName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ComponentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for ComponentName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ComponentName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ApkEntryPath(Box<str>);

impl ApkEntryPath {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ApkEntryPath {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for ApkEntryPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ApkEntryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for ApkEntryPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ApkEntryPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<ApkEntryPath> for String {
    fn from(value: ApkEntryPath) -> Self {
        value.0.into()
    }
}
