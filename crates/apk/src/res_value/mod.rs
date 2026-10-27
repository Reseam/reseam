// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod text;

use crate::error::Result;
use reseam_bytes::slice;

/// A `Res_value`: a type byte and 32 bits whose meaning the type gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResValue {
    pub kind: u8,
    pub data: u32,
}

impl ResValue {
    pub(crate) fn read(bytes: &[u8], offset: usize, section: &'static str) -> Result<Self> {
        let bytes = slice(bytes, offset, 8, section)?;
        Ok(Self::new(
            bytes[3],
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        ))
    }

    pub(crate) fn encoded(self) -> [u8; 8] {
        let [a, b, c, d] = self.data.to_le_bytes();
        [8, 0, 0, self.kind, a, b, c, d]
    }

    pub(crate) fn replace_payload(self, bytes: &mut [u8]) {
        bytes[3..8].copy_from_slice(&self.encoded()[3..]);
    }

    pub const REFERENCE: u8 = 0x01;
    pub const ATTRIBUTE: u8 = 0x02;
    pub const STRING: u8 = 0x03;
    pub const FLOAT: u8 = 0x04;
    pub const FRACTION: u8 = 0x06;
    pub const DIMENSION: u8 = 0x05;
    pub const INT_DEC: u8 = 0x10;
    pub const INT_HEX: u8 = 0x11;
    pub const INT_BOOLEAN: u8 = 0x12;
    pub const INT_COLOR_ARGB8: u8 = 0x1c;
    pub const INT_COLOR_RGB8: u8 = 0x1d;
    pub const INT_COLOR_ARGB4: u8 = 0x1e;
    pub const INT_COLOR_RGB4: u8 = 0x1f;

    pub const fn new(kind: u8, data: u32) -> Self {
        Self { kind, data }
    }

    pub const fn string(index: u32) -> Self {
        Self::new(Self::STRING, index)
    }

    pub const fn int(value: i32) -> Self {
        Self::new(Self::INT_DEC, value as u32)
    }

    pub const fn hex(value: u32) -> Self {
        Self::new(Self::INT_HEX, value)
    }

    pub const fn boolean(value: bool) -> Self {
        Self::new(Self::INT_BOOLEAN, if value { 0xFFFF_FFFF } else { 0 })
    }

    /// What aapt stores in an `id` entry. A reference to the entry resolves to
    /// the id itself, where a `@null` value would resolve it to nothing.
    pub const fn id_entry() -> Self {
        Self::boolean(false)
    }

    pub const fn reference(id: u32) -> Self {
        Self::new(Self::REFERENCE, id)
    }

    pub const fn attribute(id: u32) -> Self {
        Self::new(Self::ATTRIBUTE, id)
    }

    pub(crate) fn float(value: f32) -> Self {
        Self::new(Self::FLOAT, value.to_bits())
    }

    pub(crate) fn string_index(self) -> Option<u32> {
        (self.kind == Self::STRING).then_some(self.data)
    }

    /// A non-negative decimal or a hex integer.
    pub fn as_int(self) -> Option<u32> {
        match self.kind {
            Self::INT_DEC if (self.data as i32) >= 0 => Some(self.data),
            Self::INT_HEX => Some(self.data),
            _ => None,
        }
    }

    /// A decimal or hex integer as Android reads it, in two's complement.
    pub fn as_signed_int(self) -> Option<i32> {
        matches!(self.kind, Self::INT_DEC | Self::INT_HEX).then_some(self.data as i32)
    }

    pub fn as_bool(self) -> Option<bool> {
        (self.kind == Self::INT_BOOLEAN).then_some(self.data != 0)
    }

    /// Any of the colour kinds, as ARGB.
    pub fn color(self) -> Option<u32> {
        (Self::INT_COLOR_ARGB8..=Self::INT_COLOR_RGB4)
            .contains(&self.kind)
            .then_some(self.data)
    }
}
