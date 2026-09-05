// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

/// A `Res_value`: the typed scalar carried by AXML attributes and resource
/// entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResValue {
    pub kind: u8,
    pub data: u32,
}

impl ResValue {
    pub const REFERENCE: u8 = 0x01;
    pub const ATTRIBUTE: u8 = 0x02;
    pub const STRING: u8 = 0x03;
    pub const FLOAT: u8 = 0x04;
    pub const DIMENSION: u8 = 0x05;
    pub const INT_DEC: u8 = 0x10;
    pub const INT_HEX: u8 = 0x11;
    pub const INT_BOOLEAN: u8 = 0x12;
    pub const INT_COLOR_ARGB8: u8 = 0x1c;
    pub const INT_COLOR_RGB8: u8 = 0x1d;

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

    pub fn as_bool(self) -> Option<bool> {
        (self.kind == Self::INT_BOOLEAN).then_some(self.data != 0)
    }

    /// `#RGB`, `#ARGB`, `#RRGGBB` or `#AARRGGBB`.
    pub(crate) fn parse_color(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        let nibble = |i: usize| {
            u32::from_str_radix(&hex[i..i + 1], 16)
                .ok()
                .map(|v| v * 0x11)
        };
        Some(match hex.len() {
            3 => Self::new(
                Self::INT_COLOR_RGB8,
                0xFF00_0000 | (nibble(0)? << 16) | (nibble(1)? << 8) | nibble(2)?,
            ),
            4 => Self::new(
                Self::INT_COLOR_ARGB8,
                (nibble(0)? << 24) | (nibble(1)? << 16) | (nibble(2)? << 8) | nibble(3)?,
            ),
            6 => Self::new(
                Self::INT_COLOR_RGB8,
                0xFF00_0000 | u32::from_str_radix(hex, 16).ok()?,
            ),
            8 => Self::new(Self::INT_COLOR_ARGB8, u32::from_str_radix(hex, 16).ok()?),
            _ => return None,
        })
    }

    /// A number with a `dp`, `dip`, `sp`, `pt`, `in`, `mm` or `px` suffix,
    /// encoded as a complex dimension: unit in bits 0-3, radix in bits 4-5,
    /// mantissa in bits 8-31.
    pub(crate) fn parse_dimension(text: &str) -> Option<Self> {
        const UNITS: [(&str, u32); 7] = [
            ("dip", 1),
            ("dp", 1),
            ("sp", 2),
            ("pt", 3),
            ("in", 4),
            ("mm", 5),
            ("px", 0),
        ];
        let (number, unit) = UNITS
            .iter()
            .find_map(|(suffix, unit)| text.strip_suffix(suffix).map(|n| (n, *unit)))?;
        let value: f32 = number.parse().ok()?;
        let whole = value as i32;
        let data = if (value - whole as f32).abs() < f32::EPSILON && whole.abs() < 0x80_0000 {
            ((whole as u32) & 0xFF_FFFF) << 8 | unit
        } else {
            (((value * 128.0) as i32 as u32) & 0xFF_FFFF) << 8 | (1 << 4) | unit
        };
        Some(Self::new(Self::DIMENSION, data))
    }
}
