/// Header fields copied from the on-disk DEX header.
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
    /// Size of the entire physical file (v41+ container format).
    /// For v40 and earlier this equals `file_size`.
    pub container_size: u32,
    /// Offset from the start of the physical file to this header (v41+ container format).
    /// For v40 and earlier this is `0`.
    pub header_offset: u32,
}

/// Supported DEX format versions.
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
    /// Returns the magic bytes used in the file header for this version.
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

    /// Parses a DEX version from the eight-byte file magic.
    pub fn from_magic(magic: &[u8; 8]) -> Option<Self> {
        match magic {
            b"dex\n035\0" => Some(Self::V035),
            b"dex\n037\0" => Some(Self::V037),
            b"dex\n038\0" => Some(Self::V038),
            b"dex\n039\0" => Some(Self::V039),
            b"dex\n040\0" => Some(Self::V040),
            b"dex\n041\0" => Some(Self::V041),
            _ => None,
        }
    }

    /// Reports whether this version supports call sites and method handles.
    pub fn supports_call_sites(self) -> bool {
        self >= Self::V038
    }

    /// Reports whether this version supports hidden API metadata.
    pub fn supports_hidden_api(self) -> bool {
        self >= Self::V039
    }

    /// Reports whether this version uses the container format (v41+).
    pub fn is_container_format(self) -> bool {
        self >= Self::V041
    }

    /// Returns the expected on-disk header size for this version.
    pub fn header_size(self) -> u32 {
        if self.is_container_format() {
            0x78
        } else {
            0x70
        }
    }
}

/// Parsing configuration knobs for validation strictness and lazy loading.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub skip_checksum: bool,
    pub skip_signature: bool,
    pub lenient_leb128: bool,
    pub lenient_mutf8: bool,
    /// When true, skip parsing class_data and code items upfront.
    /// Class data can be resolved on demand via `DexFile::resolve_class_data()`.
    /// The raw buffer is retained as `Arc<[u8]>` for later resolution.
    pub lazy: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            skip_checksum: false,
            skip_signature: false,
            lenient_leb128: true,
            lenient_mutf8: true,
            lazy: false,
        }
    }
}
