// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use crate::HermesFile;
use crate::error::{HermesError, LinkError, Result};
use crate::string::StringId;

mod returned;

use returned::{Value, returned};

/// What a module declares to be linked. Its global code evaluates to `{ imports, link }`:
/// `imports` is an array literal of the specifiers of the app modules it needs, and
/// `link(modules)` returns its exports object, given an object that maps each specifier to the
/// Metro module ID it resolves to.
pub(crate) struct Interface {
    pub imports: Vec<String>,
    /// The properties every return path of `link` gives its object.
    pub exports: Vec<StringId>,
}

/// The specifiers of the app modules `module` imports, in declaration order.
pub fn imports(module: &HermesFile<'_>) -> Result<Vec<String>> {
    interface(module).map(|interface| interface.imports)
}

pub(crate) fn interface(file: &HermesFile<'_>) -> Result<Interface> {
    let declaration = returned(file, file.global_function(), &mut BTreeSet::new())?;
    let property = |name: &str| -> Result<Option<Value>> {
        for (key, value) in &declaration {
            if file.string(*key)?.equals(name) {
                return Ok(value.clone());
            }
        }
        Ok(None)
    };
    let Some(Value::Strings(imports)) = property("imports")? else {
        return Err(HermesError::Link(LinkError::Imports));
    };
    let Some(Value::Function(link)) = property("link")? else {
        return Err(HermesError::Link(LinkError::Link));
    };
    let exports = returned(file, link, &mut BTreeSet::new())?
        .into_keys()
        .collect();
    Ok(Interface { imports, exports })
}
