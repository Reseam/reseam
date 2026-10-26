// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The text forms of values, as resource XML writes them.

use super::ResValue;

impl ResValue {
    /// The text a non-string value parses back from, or `None` for a string,
    /// whose text lives in a pool, or a kind that has no text form.
    pub fn to_text(self) -> Option<String> {
        Some(match self.kind {
            Self::REFERENCE if self.data == 0 => "@null".to_string(),
            Self::REFERENCE => format!("@0x{:08x}", self.data),
            Self::ATTRIBUTE => format!("?0x{:08x}", self.data),
            Self::INT_BOOLEAN => (self.data != 0).to_string(),
            Self::INT_DEC => (self.data as i32).to_string(),
            Self::INT_HEX => format!("0x{:x}", self.data),
            Self::FLOAT => f32::from_bits(self.data).to_string(),
            Self::INT_COLOR_ARGB8..=Self::INT_COLOR_RGB4 => format!("#{:08x}", self.data),
            _ => return None,
        })
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
