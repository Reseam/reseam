// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod items;
mod validate;
mod walk;

use crate::model::Pool;

pub(crate) use items::{
    annotations_dir, call_site, debug_info, encoded_value, id_entry, instruction,
};
pub(crate) use validate::{
    ReferenceValidator, check_index, validate_class, validate_part, validate_pools,
    validate_skeleton,
};
pub(crate) use walk::{Bodies, class, class_header, raw_class};

pub(crate) trait RefSink {
    fn add(&mut self, pool: Pool, idx: u32);
}

impl<F: FnMut(Pool, u32)> RefSink for F {
    fn add(&mut self, pool: Pool, idx: u32) {
        self(pool, idx);
    }
}
