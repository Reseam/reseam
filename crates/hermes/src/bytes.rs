// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) const fn align(offset: usize) -> usize {
    (offset + 3) & !3
}
