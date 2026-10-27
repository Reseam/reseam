// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) const fn align(offset: usize) -> usize {
    (offset + 3) & !3
}
