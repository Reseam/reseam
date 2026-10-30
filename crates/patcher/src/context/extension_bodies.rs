// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod defined;
mod reachable;

use std::collections::HashSet;

use reseam_dex::{
    AccessFlags, DexFile, FieldIdx, MethodIdx, RefKey, RefQuery, TypeIdx, class_descriptor,
};

use super::PatchContext;
use crate::error::{PatcherError, Result};
use defined::{Extensions, MethodKey};
use reachable::Reachable;

/// Uses the app's own code makes of extensions: the methods it calls, and the
/// classes whose fields it reads or writes.
fn app_uses(app: &DexFile, reachable: &mut Reachable<'_>) -> Result<()> {
    let classes: HashSet<TypeIdx> = reachable
        .extensions
        .classes
        .keys()
        .filter_map(|class| app.find_type_idx(class))
        .collect();
    if classes.is_empty() {
        return Ok(());
    }
    for (index, id) in app.methods().iter().enumerate() {
        let method = MethodIdx(index as u32);
        if classes.contains(&id.class) && refers_to(app, RefKey::Method(method))? {
            reachable.call(&MethodKey::of(app, method));
        }
    }
    for (index, id) in app.fields().iter().enumerate() {
        if classes.contains(&id.class) && refers_to(app, RefKey::Field(FieldIdx(index as u32)))? {
            reachable.initialize(&app.type_descriptor(id.class));
        }
    }
    Ok(())
}

fn defines_native(dex: &DexFile) -> Result<bool> {
    let found = dex.scan_methods_find(&RefQuery::default(), |view| {
        Ok(view
            .access_flags
            .contains(AccessFlags::NATIVE)
            .then_some(()))
    })?;
    Ok(found.is_some())
}

/// Whether code in `dex` uses `key`. Pools can keep entries code no longer
/// uses, so the instructions decide.
fn refers_to(dex: &DexFile, key: RefKey) -> Result<bool> {
    let found = dex.scan_methods_find(&RefQuery::any_of([key]), |view| {
        Ok(view
            .any_instruction(|instruction| match key {
                RefKey::Method(method) => instruction.method_ref() == Some(method),
                RefKey::Field(field) => instruction.field_ref() == Some(field),
                RefKey::String(string) => instruction.string_ref() == Some(string),
                RefKey::Literal(value) => instruction.literal() == Some(value),
            })?
            .then_some(()))
    })?;
    Ok(found.is_some())
}

impl PatchContext<'_> {
    /// Fails when the patched app can call an extension method that still has
    /// no body: a native method, which a patch was to implement. Extensions
    /// cannot carry native code, so the call could only fail. Methods nothing
    /// can call stay as they are.
    pub(crate) fn check_extension_bodies(&self) -> Result<()> {
        let apk = self.apk();
        let linked: Vec<&DexFile> = (0..apk.dex().len())
            .filter(|&index| apk.is_added_dex(index))
            .filter_map(|index| apk.dex().get(index))
            .collect();
        let mut native = false;
        for dex in &linked {
            native = native || defines_native(dex)?;
        }
        if !native {
            return Ok(());
        }
        let mut extensions = Extensions::default();
        for dex in linked {
            extensions.add(dex)?;
        }
        let mut reachable = Reachable::new(&extensions);
        for component in apk.components() {
            for class in component.manifest().component_classes() {
                reachable.create_component(&class_descriptor(&class));
            }
        }
        for index in (0..apk.dex().len()).filter(|&index| !apk.is_added_dex(index)) {
            if let Some(app) = apk.dex().get(index).filter(|dex| dex.is_dirty()) {
                app_uses(app, &mut reachable)?;
            }
        }
        reachable.follow();
        let missing = reachable.unimplemented();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(PatcherError::UnimplementedExtensionMethods(missing))
        }
    }
}
