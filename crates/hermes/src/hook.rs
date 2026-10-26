// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::code::{Code, HookKind};
use crate::edit::Editor;
use crate::edit::original_function;
use crate::error::{HermesError, Result, invalid};
use crate::function::{Function, FunctionKind, Prohibit};
use crate::opcode::Op;
use crate::string::{StringId, StringKind};
use crate::{FunctionId, FunctionIndex, ModuleId};
use codegen::{Layer, Links};

mod codegen;
mod resolver;
mod root;

use resolver::Resolver;

impl Editor<'_> {
    /// Adds `code` to `function` as a new layer; callers are untouched. [`HookKind::Before`] and
    /// [`HookKind::After`] run around the function as it was, which later layers then run around
    /// in turn; [`HookKind::Replace`] runs instead of it, and earlier layers no longer run. The
    /// receiver and the linked modules' exports keep the function's original captured
    /// environment.
    ///
    /// Generators, async functions, constructors, functions using `new.target` or direct
    /// `eval`, unknown exports, malformed code and unprovable environment chains fail. A replaced
    /// function whose closures an earlier hook needs fails too. Hooked functions cannot be used
    /// as constructors afterwards.
    pub fn hook(
        &mut self,
        index: &FunctionIndex,
        function: FunctionId,
        kind: HookKind,
        code: &Code,
    ) -> Result<()> {
        index.check(&self.file)?;
        self.transaction(|editor| editor.hook_function(index, function, kind, code))
    }

    fn hook_function(
        &mut self,
        index: &FunctionIndex,
        function: FunctionId,
        kind: HookKind,
        code: &Code,
    ) -> Result<()> {
        let target = self.editable(function)?;
        let mut resolver = Resolver {
            editor: self,
            kind,
            slots: 0,
            calls_exports: false,
        };
        let code = resolver.code(code)?;
        let (slots, calls_exports) = (resolver.slots, resolver.calls_exports);
        let plan = if calls_exports {
            Some(self.hook_plan(index, function)?)
        } else {
            None
        };
        if let Some(id) = plan
            .iter()
            .flat_map(|plan| plan.roots.keys())
            .find(|id| self.edits.discarded.contains(id) && !self.edits.relocated.contains_key(id))
        {
            return Err(HermesError::Unsupported(format!(
                "function {} was replaced, so the closures this hook needs no longer exist",
                id.0
            )));
        }
        let edited = self.edits.functions.contains_key(&function);
        let original = (kind != HookKind::Replace).then(|| self.next_function());
        let layer = Layer { kind, code, slots };
        let links = Links {
            original,
            modules: plan.as_ref().map(|plan| plan.depth),
        };
        let body = codegen::compile(&target, &layer, &links)?;
        if let Some(original) = original {
            let previous = if edited {
                self.edits.functions[&function].clone()
            } else {
                original_function(&target)?
            };
            self.append_function(previous);
            if !edited {
                self.edits.relocate(function, original);
            }
        } else if !edited {
            if self.edits.roots.contains_key(&function) {
                return Err(HermesError::Unsupported(format!(
                    "function {} creates closures that a hook needs",
                    function.0
                )));
            }
            self.edits.discard(function);
        }
        self.edits.set_function(function, body);
        for (id, root) in plan.into_iter().flat_map(|plan| plan.roots) {
            self.edits.attach_root(id, root);
        }
        Ok(())
    }

    fn declared_export(&mut self, module: ModuleId, export: &str) -> Result<StringId> {
        let linked = self
            .edits
            .modules
            .get(module.0 as usize)
            .ok_or_else(|| invalid(0, "module belongs to another editor"))?;
        let declared = linked.exports.iter().any(|id| {
            self.edits.strings[(id.0 - self.file.string_count()) as usize]
                .value()
                .equals(export)
        });
        if !declared {
            return Err(HermesError::Unsupported(format!(
                "module does not export {export:?}"
            )));
        }
        self.intern(export, StringKind::Identifier)
    }
}

impl<'a> Editor<'a> {
    /// The app function whose body an edit may replace. Generators, async functions and
    /// constructors keep state or a receiver that a replaced body cannot reproduce.
    pub(crate) fn editable(&self, function: FunctionId) -> Result<Function<'a>> {
        if function == self.file.global_function() {
            return Err(HermesError::Unsupported("global function".into()));
        }
        let target = self.file.function(function)?;
        if target.header.flags.kind != FunctionKind::Normal
            || target.header.flags.prohibit == Prohibit::Call
            || target
                .instructions()?
                .iter()
                .any(|i| matches!(i.op, Op::GetNewTarget | Op::DirectEval))
        {
            return Err(HermesError::Unsupported(format!(
                "function {} is a generator, async function, constructor or uses new.target/eval",
                function.0
            )));
        }
        Ok(target)
    }
}

#[cfg(test)]
mod tests;
