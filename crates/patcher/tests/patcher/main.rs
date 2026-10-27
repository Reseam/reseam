// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#![expect(clippy::unwrap_used, reason = "fixture operations must succeed")]

mod app_entry;
mod common;
mod emitted;
mod extensions;
mod points;
mod queries;
mod resources;
mod runtime;
mod settings;
