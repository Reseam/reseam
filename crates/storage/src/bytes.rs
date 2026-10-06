// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
enum Storage {
    Owned(Arc<Vec<u8>>),
    Mapped(Arc<crate::MappedFile>),
}

#[derive(Clone)]
pub struct Bytes {
    storage: Storage,
    len: usize,
}

impl Default for Bytes {
    fn default() -> Self {
        Self::from_vec(Vec::new())
    }
}

impl Bytes {
    pub fn from_slice(buf: &[u8]) -> Self {
        Self::from_vec(buf.to_vec())
    }

    pub fn from_vec(buf: Vec<u8>) -> Self {
        let len = buf.len();
        Self {
            storage: Storage::Owned(Arc::new(buf)),
            len,
        }
    }

    pub fn from_mmap(mmap: Arc<crate::MappedFile>) -> Self {
        let len = mmap.len();
        Self {
            storage: Storage::Mapped(mmap),
            len,
        }
    }

    /// Restricts the visible prefix without copying. Returns `None` if the
    /// requested length exceeds the current prefix.
    pub fn bounded(mut self, len: usize) -> Option<Self> {
        if len > self.len {
            return None;
        }
        self.len = len;
        Some(self)
    }

    /// Whether views share an allocation or mapping, regardless of visible prefix.
    #[cfg_attr(
        target_os = "wasi",
        expect(
            clippy::match_same_arms,
            reason = "copied WASI mappings and owned bytes retain separate storage identities"
        )
    )]
    pub fn same_source(&self, other: &Self) -> bool {
        match (&self.storage, &other.storage) {
            (Storage::Owned(a), Storage::Owned(b)) => Arc::ptr_eq(a, b),
            (Storage::Mapped(a), Storage::Mapped(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self.storage {
            Storage::Owned(b) => &b[..self.len],
            Storage::Mapped(m) => &m[..self.len],
        }
    }

    /// Drops the resident pages of a mapped file after a pass over all of
    /// it, so a large file stays resident only while it is being read. The
    /// pages come back from the page cache on the next access.
    #[cfg_attr(
        not(unix),
        expect(
            clippy::unused_self,
            reason = "page eviction requires native Unix memory maps"
        )
    )]
    pub fn release_pages(&self) {
        #[cfg(unix)]
        if let Storage::Mapped(map) = &self.storage {
            // SAFETY: the mapping is read-only, so a dropped page reads back
            // unchanged from the file.
            // Page eviction is a best-effort hint; failure leaves the mapping usable.
            drop(unsafe { map.unchecked_advise(memmap2::UncheckedAdvice::DontNeed) });
        }
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.storage {
            Storage::Owned(b) => f.debug_tuple("Owned").field(&b.len()).finish(),
            Storage::Mapped(m) => f.debug_tuple("Mapped").field(&m.len()).finish(),
        }
    }
}
