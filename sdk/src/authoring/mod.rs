// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Making bundles: a new project from the template, the signing key, the
//! staging manifest, and packing.

mod template;

pub use reseam_patcher::bundle::{BundleInfo, BundleKey, manifest_info, pack};
pub use template::{TEMPLATE_URL, init_bundle};
