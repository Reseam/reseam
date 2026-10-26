// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::closure::Closures;
use crate::code::{Code, HookKind};
use crate::edit::{Edits, ModuleId, original_function};
use crate::error::{Result, Unsupported, unsupported};
use crate::function::{Function, FunctionKind, Prohibit};
use crate::opcode::Op;
use crate::string::{StringId, StringKind};
use crate::{FunctionId, FunctionIndex, HermesFile};
use codegen::{Layer, Links};

mod codegen;
mod resolver;
mod root;

use resolver::Resolver;
pub(crate) use root::rooted_functions;

/// Hooks app functions of one file: its edits, and the closure analysis that finds where linked
/// modules' exports reach them.
pub(crate) struct Hooker<'e> {
    pub file: HermesFile<'e>,
    pub index: &'e FunctionIndex,
    pub closures: &'e mut Closures,
    pub edits: &'e mut Edits,
}

impl Hooker<'_> {
    pub(crate) fn hook(&mut self, function: FunctionId, kind: HookKind, code: &Code) -> Result<()> {
        let target = editable(&self.file, function)?;
        let mut resolver = Resolver {
            function,
            file: &self.file,
            edits: &mut *self.edits,
            kind,
            slots: 0,
            calls_exports: false,
        };
        let code = resolver.code(code)?;
        let (slots, calls_exports) = (resolver.slots, resolver.calls_exports);
        let plan = if calls_exports {
            Some(self.closures.plan(&self.file, self.index, function)?)
        } else {
            None
        };
        if let Some(id) = plan
            .iter()
            .flat_map(|plan| plan.roots.keys())
            .find(|&&id| self.edits.is_discarded(id))
        {
            return Err(unsupported(*id, Unsupported::Replaced));
        }
        let original =
            (kind != HookKind::Replace).then(|| FunctionId(self.edits.function_count(&self.file)));
        let layer = Layer { kind, code, slots };
        let links = Links {
            original,
            modules: plan.as_ref().map(|plan| plan.depth),
        };
        let body = codegen::compile(&target, &layer, &links)?;
        let edited = self.edits.function(&self.file, function).is_some();
        if let Some(original) = original {
            let previous = match self.edits.function(&self.file, function) {
                Some(previous) => previous.clone(),
                None => original_function(&target)?,
            };
            self.edits.append_function(&self.file, previous);
            if !edited {
                self.edits.relocate(function, original);
            }
        } else if !edited {
            if self.edits.roots().contains_key(&function) {
                return Err(unsupported(function, Unsupported::NeededClosures));
            }
            self.edits.discard(function);
        }
        self.edits.replace(function, body);
        for (id, root) in plan.into_iter().flat_map(|plan| plan.roots) {
            self.edits.attach_root(id, root);
        }
        Ok(())
    }
}

/// The app function whose body an edit may replace. Generators, async functions and
/// constructors keep state or a receiver that a replaced body cannot reproduce.
fn editable<'a>(file: &HermesFile<'a>, function: FunctionId) -> Result<Function<'a>> {
    if function == file.global_function() {
        return Err(unsupported(function, Unsupported::Global));
    }
    let target = file.function(function)?;
    if target.header.flags.kind != FunctionKind::Normal
        || target.header.flags.prohibit == Prohibit::Call
        || target
            .instructions()?
            .iter()
            .any(|i| matches!(i.op, Op::GetNewTarget | Op::DirectEval))
    {
        return Err(unsupported(function, Unsupported::Stateful));
    }
    Ok(target)
}

/// The string naming `export` of a linked `module`, which must declare it.
fn declared_export(
    file: &HermesFile<'_>,
    edits: &mut Edits,
    function: FunctionId,
    module: ModuleId,
    export: &str,
) -> Result<StringId> {
    let linked = edits
        .modules()
        .get(module.0 as usize)
        .ok_or_else(|| unsupported(function, Unsupported::Module))?;
    let mut declared = false;
    for &id in &linked.exports {
        declared |= edits.strings().string(file, id)?.equals(export);
    }
    if !declared {
        return Err(unsupported(
            function,
            Unsupported::Export(export.to_owned()),
        ));
    }
    edits
        .strings_mut()
        .intern(file, export, StringKind::Identifier)
}

#[cfg(test)]
mod tests;
