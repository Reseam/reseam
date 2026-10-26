// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::dataflow::register_states;
use crate::error::{Result, Unsupported, unsupported};
use crate::opcode::Op;
use crate::string::StringId;
use crate::{FunctionId, FunctionIndex, HermesFile};

mod exports;

pub(crate) use exports::ModuleShape;

/// Metro's module registration global, `__d(factory, moduleId, dependencyMap)`.
const DEFINE: &str = "__d";

/// A module ID that Metro's `__r` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MetroModuleId(pub u32);

/// The Metro modules the app's global code defines, by factory function, read on first use.
#[derive(Default)]
pub(crate) struct MetroModules {
    factories: Option<BTreeMap<FunctionId, MetroModuleId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    Define,
    Closure(FunctionId),
    Id(MetroModuleId),
}

impl MetroModules {
    /// The Metro module whose factory creates `function`, directly or through enclosing
    /// functions, and what it exports. A function created outside every module, or inside
    /// several, fails.
    pub(crate) fn module(
        &mut self,
        file: &HermesFile<'_>,
        index: &FunctionIndex,
        function: FunctionId,
    ) -> Result<(MetroModuleId, ModuleShape)> {
        if self.factories.is_none() {
            self.factories = Some(factories(file)?);
        }
        let factories = self.factories.as_ref().expect("factories are read above");
        let global = file.global_function();
        let mut modules = BTreeMap::new();
        let mut seen = BTreeSet::from([function]);
        let mut pending = VecDeque::from([function]);
        while let Some(id) = pending.pop_front() {
            if let Some(&module) = factories.get(&id) {
                modules.entry(module).or_insert(id);
                continue;
            }
            if id == global {
                return Err(unsupported(function, Unsupported::OutsideMetro));
            }
            for &creator in index.creators(id) {
                if seen.insert(creator) {
                    pending.push_back(creator);
                }
            }
        }
        match modules.into_iter().collect::<Vec<_>>()[..] {
            [(module, factory)] => Ok((module, exports::shape(file, factory)?)),
            [] => Err(unsupported(function, Unsupported::Unreachable)),
            _ => Err(unsupported(function, Unsupported::SeveralMetro)),
        }
    }
}

/// Reads every `__d` call in the global function. A call whose factory or module ID is not a
/// value the analysis can prove fails, so no module is silently left out.
fn factories(file: &HermesFile<'_>) -> Result<BTreeMap<FunctionId, MetroModuleId>> {
    let global = file.function(file.global_function())?;
    let code = global.instructions()?;
    let states = register_states(&global, &code, |inst, before| {
        let v = &inst.values;
        Ok(match inst.op {
            Op::TryGetById | Op::TryGetByIdLong
                if file.string(StringId(v[3] as u32))?.equals(DEFINE) =>
            {
                Some(Known::Define)
            }
            Op::CreateClosure | Op::CreateClosureLongIndex => {
                Some(Known::Closure(FunctionId(v[2] as u32)))
            }
            Op::LoadConstZero => Some(Known::Id(MetroModuleId(0))),
            Op::LoadConstUInt8 => Some(Known::Id(MetroModuleId(v[1] as u32))),
            Op::LoadConstInt => u32::try_from(v[1] as u32 as i32)
                .ok()
                .map(|id| Known::Id(MetroModuleId(id))),
            Op::Mov | Op::MovLong => before.get(&(v[1] as u32)).copied(),
            _ => None,
        })
    })?;
    let mut factories = BTreeMap::new();
    for (index, inst) in code.iter().enumerate() {
        let Some(state) = states.get(&index) else {
            continue;
        };
        if inst.op != Op::Call4 || state.get(&(inst.values[1] as u32)) != Some(&Known::Define) {
            continue;
        }
        let (Some(&Known::Closure(factory)), Some(&Known::Id(id))) = (
            state.get(&(inst.values[3] as u32)),
            state.get(&(inst.values[4] as u32)),
        ) else {
            let offset = inst.offset.expect("analysed instructions are decoded");
            return Err(unsupported(global.id, Unsupported::UnprovenMetro(offset)));
        };
        if factories
            .insert(factory, id)
            .is_some_and(|previous| previous != id)
        {
            return Err(unsupported(factory, Unsupported::SharedFactory));
        }
    }
    if factories.is_empty() {
        return Err(unsupported(global.id, Unsupported::NoMetro));
    }
    Ok(factories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{HOST, image};

    #[test]
    fn functions_resolve_to_the_metro_module_whose_factory_creates_them() {
        let host = image(HOST);
        let file = host.file();
        let index = FunctionIndex::build(&file).expect("host indexes");
        let mut modules = MetroModules::default();
        let cases: [(&str, u32, bool, &[&str]); 4] = [
            ("greet", 7, false, &["greet"]),
            ("getCount", 300, false, &["store"]),
            ("getTheme", 301, true, &["ThemeTypes", "default"]),
            ("View", 302, false, &["Text", "View"]),
        ];
        for (name, id, es_module, keys) in cases {
            let function = index
                .find_function(&file, Some(name), &[], None)
                .expect("fixture function exists");
            let (module, shape) = modules
                .module(&file, &index, function)
                .expect("module resolves");
            let mut names: Vec<String> = shape
                .keys
                .iter()
                .map(|key| {
                    file.string(*key)
                        .expect("key exists")
                        .text()
                        .expect("key is text")
                })
                .collect();
            names.sort();
            assert_eq!(
                (module, shape.es_module, names),
                (
                    MetroModuleId(id),
                    es_module,
                    keys.iter().map(ToString::to_string).collect()
                )
            );
        }
        assert!(
            modules
                .module(&file, &index, file.global_function())
                .is_err()
        );
    }
}
