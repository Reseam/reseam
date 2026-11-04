// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Making bundles: a new project from the template, the signing key, the
//! staging manifest, packing, and signing a bundle packed without a key.

mod template;

pub use reseam_patcher::bundle::{BundleInfo, BundleKey, manifest_info, pack, sign};
pub use template::{TEMPLATE_URL, init_bundle};
