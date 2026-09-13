// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub enum ApplicationIcon {
    /// An encoded PNG, WebP or JPEG.
    Bitmap(Vec<u8>),
    /// Layers on a 108dp canvas of which a launcher shows the central 72dp.
    Adaptive {
        background: IconLayer,
        foreground: IconLayer,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub enum IconLayer {
    Bitmap(Vec<u8>),
    /// ARGB.
    Color(u32),
}
