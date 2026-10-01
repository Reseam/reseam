// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone)]
pub struct DexHeader {
    pub version: DexVersion,
    pub checksum: u32,
    pub signature: [u8; 20],
    pub file_size: u32,
    pub link_size: u32,
    pub link_off: u32,
    pub map_off: u32,
    pub string_ids_size: u32,
    pub string_ids_off: u32,
    pub type_ids_size: u32,
    pub type_ids_off: u32,
    pub proto_ids_size: u32,
    pub proto_ids_off: u32,
    pub field_ids_size: u32,
    pub field_ids_off: u32,
    pub method_ids_size: u32,
    pub method_ids_off: u32,
    pub class_defs_size: u32,
    pub class_defs_off: u32,
    pub data_size: u32,
    pub data_off: u32,
    pub container_size: u32,
    pub header_offset: u32,
}

impl DexHeader {
    /// Empty header for a newly authored DEX; serialization derives all sizes and offsets.
    pub fn new(version: DexVersion) -> Self {
        Self {
            version,
            checksum: 0,
            signature: [0; 20],
            file_size: 0,
            link_size: 0,
            link_off: 0,
            map_off: 0,
            string_ids_size: 0,
            string_ids_off: 0,
            type_ids_size: 0,
            type_ids_off: 0,
            proto_ids_size: 0,
            proto_ids_off: 0,
            field_ids_size: 0,
            field_ids_off: 0,
            method_ids_size: 0,
            method_ids_off: 0,
            class_defs_size: 0,
            class_defs_off: 0,
            data_size: 0,
            data_off: 0,
            container_size: 0,
            header_offset: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DexVersion {
    V035,
    V037,
    V038,
    V039,
    V040,
    V041,
}

impl DexVersion {
    pub fn magic_bytes(self) -> &'static [u8; 8] {
        match self {
            Self::V035 => b"dex\n035\0",
            Self::V037 => b"dex\n037\0",
            Self::V038 => b"dex\n038\0",
            Self::V039 => b"dex\n039\0",
            Self::V040 => b"dex\n040\0",
            Self::V041 => b"dex\n041\0",
        }
    }

    pub fn from_magic(magic: [u8; 8]) -> Option<Self> {
        match &magic {
            b"dex\n035\0" => Some(Self::V035),
            b"dex\n037\0" => Some(Self::V037),
            b"dex\n038\0" => Some(Self::V038),
            b"dex\n039\0" => Some(Self::V039),
            b"dex\n040\0" => Some(Self::V040),
            b"dex\n041\0" => Some(Self::V041),
            _ => None,
        }
    }

    pub fn supports_call_sites(self) -> bool {
        self >= Self::V038
    }

    pub fn supports_hidden_api(self) -> bool {
        self >= Self::V039
    }

    pub fn is_container_format(self) -> bool {
        self >= Self::V041
    }

    pub fn header_size(self) -> u32 {
        if self.is_container_format() {
            0x78
        } else {
            0x70
        }
    }
}

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
