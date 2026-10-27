// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod access;
mod slots;
mod space;
mod table;

/// The handle a location gets when it cannot have one; never valid.
const INVALID_HANDLE: u32 = u32::MAX;

pub(super) use access::{
    alloc_class, alloc_method, alloc_methods, class_location, forget_class, forget_method,
    method_location, method_ref, relocate_method, reset, with_class_mut, with_code, with_handles,
    with_instruction, with_method_mut,
};
pub(super) use space::HandleSpace;
