// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buf::slice;
use crate::error::Result;

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

    pub fn as_bool(self) -> Option<bool> {
        (self.kind == Self::INT_BOOLEAN).then_some(self.data != 0)
    }

    /// Any of the colour kinds, as ARGB.
    pub fn color(self) -> Option<u32> {
        (Self::INT_COLOR_ARGB8..=Self::INT_COLOR_RGB4)
            .contains(&self.kind)
            .then_some(self.data)
    }

    pub(crate) fn parse_color(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !hex.is_ascii() {
            return None;
        }
        let nibble = |i: usize| u32::from_str_radix(&hex[i..=i], 16).ok().map(|v| v * 0x11);
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
        Self::complex(number.parse().ok()?, unit).map(|data| Self::new(Self::DIMENSION, data))
    }

    pub(crate) fn parse_fraction(text: &str) -> Option<Self> {
        let (number, unit) = text
            .strip_suffix("%p")
            .map(|number| (number, 1))
            .or_else(|| text.strip_suffix('%').map(|number| (number, 0)))?;
        Self::complex(number.parse::<f32>().ok()? / 100.0, unit)
            .map(|data| Self::new(Self::FRACTION, data))
    }

    fn complex(value: f32, unit: u32) -> Option<u32> {
        if !value.is_finite() || !(-8_388_608.0..8_388_608.0).contains(&value) {
            return None;
        }
        if value.fract() == 0.0 {
            return Some(((value as i32 as u32) & 0xff_ffff) << 8 | unit);
        }
        let legacy = f64::from(value) * 128.0;
        if legacy.fract() == 0.0 && (-8_388_608.0..=8_388_607.0).contains(&legacy) {
            return Some(((legacy as i32 as u32) & 0xff_ffff) << 8 | 1 << 4 | unit);
        }
        [(3, 8_388_608.0), (2, 32_768.0), (1, 128.0), (0, 1.0)]
            .into_iter()
            .find_map(|(radix, scale)| {
                let mantissa = (f64::from(value) * scale).round();
                (-8_388_608.0..=8_388_607.0)
                    .contains(&mantissa)
                    .then_some(((mantissa as i32 as u32) & 0xff_ffff) << 8 | radix << 4 | unit)
            })
    }
}
