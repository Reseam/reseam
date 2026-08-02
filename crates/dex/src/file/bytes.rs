// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub enum DexBytes {
    Owned(Arc<Vec<u8>>),
    Mapped(Arc<memmap2::Mmap>),
}

impl DexBytes {
    pub fn from_slice(buf: &[u8]) -> Self {
        Self::Owned(Arc::new(buf.to_vec()))
    }

    pub fn from_vec(buf: Vec<u8>) -> Self {
        Self::Owned(Arc::new(buf))
    }

    pub fn from_mmap(mmap: Arc<memmap2::Mmap>) -> Self {
        Self::Mapped(mmap)
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Owned(b) => b.as_slice(),
            Self::Mapped(m) => m.as_ref(),
        }
    }
}

impl AsRef<[u8]> for DexBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Deref for DexBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl std::fmt::Debug for DexBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned(b) => f.debug_tuple("Owned").field(&b.len()).finish(),
            Self::Mapped(m) => f.debug_tuple("Mapped").field(&m.len()).finish(),
        }
    }
}
