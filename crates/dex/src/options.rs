// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

/// Entries an id pool other than strings can hold: its indices are 16-bit.
pub const MAX_POOL_SIZE: usize = 1 << 16;

/// Controls validation and materialization without discarding source metadata.
#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    pub checksum: Verification,
    pub signature: Verification,
    pub leb128: Validation,
    pub mutf8: Validation,
    pub classes: Loading,
    pub debug_info: Loading,
    pub annotations: Loading,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            checksum: Verification::Required,
            signature: Verification::Required,
            leb128: Validation::Permissive,
            mutf8: Validation::Permissive,
            classes: Loading::Eager,
            debug_info: Loading::Eager,
            annotations: Loading::Eager,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
    Required,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validation {
    Strict,
    Permissive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loading {
    Eager,
    Deferred,
}

/// Selection of metadata to emit, independent of how it was loaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetadataPolicy {
    #[default]
    Preserve,
    Omit,
    /// Omit metadata from this DEX's original mapping, retaining imported or authored metadata.
    OmitOriginal,
}

/// Opaque link bytes cannot be relocated after pool and section layout changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LinkPolicy {
    #[default]
    Reject,
    Omit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    pub debug_info: MetadataPolicy,
    pub annotations: MetadataPolicy,
    pub link_data: LinkPolicy,
}
