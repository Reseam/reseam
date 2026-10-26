// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod access_flags;
pub(crate) mod annotation;
pub(crate) mod class;
pub(crate) mod code;
pub(crate) mod debug;
pub(crate) mod descriptor;
pub(crate) mod encoded_value;
pub(crate) mod header;
pub(crate) mod hidden_api;
mod ids;
pub(crate) mod map;
pub(crate) mod metadata;
pub(crate) mod method_handle;

pub use ids::{
    CallSiteIdx, FieldId, FieldIdx, MethodHandleIdx, MethodId, MethodIdx, ProtoIdx, Prototype,
    StringIdx, TypeIdx, TypeList,
};
pub(crate) use ids::{PerPool, Pool};
