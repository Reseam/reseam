// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::FunctionId;
use crate::metro::MetroModuleId;
use crate::string::StringId;

/// Identity of a module linked into this editor. Its exports remain in a
/// private environment and are initialized before the app's global code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleId(pub(crate) u32);

/// A linked module: its global function, what each import resolves to, and its exports.
pub(crate) struct LinkedModule {
    pub global: FunctionId,
    pub imports: Vec<LinkedImport>,
    pub exports: Vec<StringId>,
}

/// An import as `link` receives it: `{ id, esModule, keys }` under the specifier, enough to read
/// the Metro module lazily, the first time the module's exports are read.
pub(crate) struct LinkedImport {
    pub specifier: StringId,
    pub id: MetroModuleId,
    pub es_module: bool,
    pub keys: Vec<StringId>,
}

/// The property names of a linked module's interface and of each import record.
#[derive(Clone, Copy)]
pub(crate) struct LinkNames {
    pub link: StringId,
    pub id: StringId,
    pub es_module: StringId,
    pub keys: StringId,
}
