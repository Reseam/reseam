// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::dataflow::register_states;
use crate::edit::Editor;
use crate::error::{HermesError, Result, invalid};
use crate::literal::shape_keys;
use crate::model::StringId;
use crate::opcode::Op;
use crate::{FunctionId, FunctionIndex, HermesFile};

/// Metro's module registration global, `__d(factory, moduleId, dependencyMap)`.
const DEFINE: &str = "__d";

/// A module ID that Metro's `__r` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MetroModuleId(pub u32);

/// The Metro modules the app's global code defines, by factory function.
pub(crate) struct MetroModules {
    factories: BTreeMap<FunctionId, MetroModuleId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    Define,
    Closure(FunctionId),
    Id(MetroModuleId),
}

impl MetroModules {
    /// Reads every `__d` call in the global function. A call whose factory or module ID is not a
    /// value the analysis can prove fails, so no module is silently left out.
    pub(crate) fn build(file: &HermesFile<'_>) -> Result<Self> {
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
            let offset = inst.offset.expect("analysed instructions are decoded") as usize;
            let (Some(&Known::Closure(factory)), Some(&Known::Id(id))) = (
                state.get(&(inst.values[3] as u32)),
                state.get(&(inst.values[4] as u32)),
            ) else {
                return Err(invalid(
                    offset,
                    "Metro module definition without a provable factory and ID",
                ));
            };
            if factories
                .insert(factory, id)
                .is_some_and(|previous| previous != id)
            {
                return Err(invalid(offset, "Metro factory defines several modules"));
            }
        }
        if factories.is_empty() {
            return Err(HermesError::Unsupported(
                "the global code defines no Metro modules".into(),
            ));
        }
        Ok(Self { factories })
    }
}

impl Editor<'_> {
    /// The Metro module whose factory creates `function`, directly or through enclosing
    /// functions. A function created outside every module, or inside several, fails.
    pub(crate) fn metro_module(
        &mut self,
        index: &FunctionIndex,
        function: FunctionId,
    ) -> Result<MetroModuleId> {
        if self.edits.metro.is_none() {
            self.edits.metro = Some(MetroModules::build(&self.file)?);
        }
        let global = self.file.global_function();
        let mut modules = BTreeSet::new();
        let mut seen = BTreeSet::from([function]);
        let mut pending = VecDeque::from([function]);
        while let Some(id) = pending.pop_front() {
            let factories = &self
                .edits
                .metro
                .as_ref()
                .expect("Metro modules built")
                .factories;
            if let Some(&module) = factories.get(&id) {
                modules.insert(module);
                continue;
            }
            if id == global {
                return Err(HermesError::Unsupported(format!(
                    "function {} is created outside every Metro module",
                    function.0
                )));
            }
            for &creator in index.creators(id) {
                if seen.insert(creator) {
                    pending.push_back(creator);
                }
            }
        }
        match modules.into_iter().collect::<Vec<_>>()[..] {
            [module] => Ok(module),
            [] => Err(HermesError::Unsupported(format!(
                "function {} is not reachable from a Metro module",
                function.0
            ))),
            _ => Err(HermesError::Unsupported(format!(
                "function {} is created in several Metro modules",
                function.0
            ))),
        }
    }
}

/// What a Metro module exports, read from its factory: the property names it gives `exports`, or
/// the object it assigns to `module.exports`, and whether it marks itself a Babel ES module.
pub(crate) struct ModuleShape {
    pub es_module: bool,
    pub keys: BTreeSet<StringId>,
}

/// Metro's factory parameters, after `this`: `global, require, importDefault, importAll, module,
/// exports, dependencyMap`.
const MODULE_PARAMETER: u64 = 5;
const EXPORTS_PARAMETER: u64 = 6;
const ES_MODULE: &str = "__esModule";

#[derive(Clone, Copy)]
enum Shaped {
    Module,
    Exports,
    Object(usize),
    Text(StringId),
}

impl MetroModules {
    fn factory(&self, id: MetroModuleId) -> Option<FunctionId> {
        self.factories
            .iter()
            .find_map(|(&factory, &module)| (module == id).then_some(factory))
    }
}

impl Editor<'_> {
    /// The Metro module whose factory creates `function`, and what it exports.
    pub(crate) fn metro_shape(
        &mut self,
        index: &FunctionIndex,
        function: FunctionId,
    ) -> Result<(MetroModuleId, ModuleShape)> {
        let id = self.metro_module(index, function)?;
        let factory = self
            .edits
            .metro
            .as_ref()
            .and_then(|modules| modules.factory(id))
            .expect("a resolved Metro module has a factory");
        Ok((id, shape(&self.file, factory)?))
    }
}

fn shape(file: &HermesFile<'_>, factory: FunctionId) -> Result<ModuleShape> {
    let mut registers = BTreeMap::<u32, Shaped>::new();
    let mut objects = Vec::<BTreeSet<StringId>>::new();
    let mut exports = BTreeSet::new();
    let mut replaced = None;
    let mut es_module = false;
    for inst in file.function(factory)?.instructions()? {
        let v = &inst.values;
        let register = |operand: usize| registers.get(&(v[operand] as u32)).copied();
        let value = match inst.op {
            Op::LoadParam if v[1] == MODULE_PARAMETER => Some(Shaped::Module),
            Op::LoadParam if v[1] == EXPORTS_PARAMETER => Some(Shaped::Exports),
            Op::NewObject => {
                objects.push(BTreeSet::new());
                Some(Shaped::Object(objects.len() - 1))
            }
            Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => {
                let keys = shape_keys(file, v[1] as u32)?
                    .iter()
                    .filter_map(|key| key.string().transpose())
                    .collect::<Result<_>>()?;
                objects.push(keys);
                Some(Shaped::Object(objects.len() - 1))
            }
            Op::LoadConstString | Op::LoadConstStringLongIndex => {
                Some(Shaped::Text(StringId(v[1] as u32)))
            }
            Op::Mov | Op::MovLong => register(1),
            _ => None,
        };
        let assigned = match inst.op {
            Op::PutByIdLoose
            | Op::PutByIdStrict
            | Op::PutByIdLooseLong
            | Op::PutByIdStrictLong
            | Op::DefineOwnById
            | Op::DefineOwnByIdLong => Some(StringId(v[3] as u32)),
            Op::DefineOwnGetterSetterByVal
            | Op::PutByValLoose
            | Op::PutByValStrict
            | Op::DefineOwnByVal => match register(1) {
                Some(Shaped::Text(key)) => Some(key),
                _ => None,
            },
            _ => None,
        };
        // Babel marks an ES module with `Object.defineProperty(exports, "__esModule", ...)`.
        if inst.op == Op::Call4
            && let (Some(Shaped::Exports), Some(Shaped::Text(key))) = (register(3), register(4))
        {
            es_module |= file.string(key)?.equals(ES_MODULE);
        }
        if let Some(key) = assigned {
            match (register(0), register(1)) {
                (Some(Shaped::Module), Some(Shaped::Object(object)))
                    if file.string(key)?.equals("exports") =>
                {
                    replaced = Some(object);
                }
                (Some(Shaped::Exports), _) if file.string(key)?.equals(ES_MODULE) => {
                    es_module = true;
                }
                (Some(Shaped::Exports), _) => {
                    exports.insert(key);
                }
                (Some(Shaped::Object(object)), _) => {
                    objects[object].insert(key);
                }
                _ => {}
            }
        }
        for written in inst.written_registers() {
            registers.remove(&written);
        }
        if let Some(value) = value {
            registers.insert(v[0] as u32, value);
        }
    }
    Ok(ModuleShape {
        es_module,
        keys: replaced.map_or(exports, |object| std::mem::take(&mut objects[object])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{HOST, function, image};

    #[test]
    fn functions_resolve_to_the_metro_module_whose_factory_creates_them() {
        let host = image(HOST);
        let index = FunctionIndex::build(&host.file()).expect("host indexes");
        let function = |name| function(&host, &index, name);
        let mut editor = Editor::new(host.file());
        assert_eq!(
            editor.metro_module(&index, function("greet")).unwrap(),
            MetroModuleId(7)
        );
        assert_eq!(
            editor.metro_module(&index, function("getCount")).unwrap(),
            MetroModuleId(300)
        );
        assert!(
            editor
                .metro_module(&index, host.file().global_function())
                .is_err()
        );
    }

    #[test]
    fn module_shapes_name_what_each_factory_exports() {
        let host = image(HOST);
        let file = host.file();
        let index = FunctionIndex::build(&file).expect("host indexes");
        let mut editor = Editor::new(host.file());
        let cases: [(&str, bool, &[&str]); 3] = [
            ("greet", false, &["greet"]),
            ("getTheme", true, &["ThemeTypes", "default"]),
            ("View", false, &["Text", "View"]),
        ];
        for (name, es_module, keys) in cases {
            let function = function(&host, &index, name);
            let (_, shape) = editor
                .metro_shape(&index, function)
                .expect("module shape reads");
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
                (shape.es_module, names),
                (es_module, keys.iter().map(ToString::to_string).collect())
            );
        }
    }
}
