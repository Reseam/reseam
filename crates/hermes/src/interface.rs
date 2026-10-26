// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{HermesError, Result};
use crate::literal::{LiteralValue, read_literals, shape_keys};
use crate::model::{Section, StringId};
use crate::opcode::{Instruction, Op};
use crate::{FunctionId, HermesFile};

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
        return Err(HermesError::Unsupported(
            "module declares no imports array of string literals".into(),
        ));
    };
    let Some(Value::Function(link)) = property("link")? else {
        return Err(HermesError::Unsupported(
            "module declares no link function".into(),
        ));
    };
    let exports = returned(file, link, &mut BTreeSet::new())?
        .into_keys()
        .collect();
    Ok(Interface { imports, exports })
}

#[derive(Clone)]
enum Value {
    Object(usize),
    Function(FunctionId),
    Strings(Vec<String>),
}

#[derive(Default)]
struct Object {
    /// The key of each slot an object literal's shape lays out.
    slots: Vec<Option<StringId>>,
    properties: BTreeMap<StringId, Option<Value>>,
}

type Properties = BTreeMap<StringId, Option<Value>>;

/// The properties of the object `id` returns: the string keys of an object literal and properties
/// assigned before any branch, kept only when every return path has them. Values the analysis
/// cannot follow are `None`; objects are not followed past a return.
fn returned(
    file: &HermesFile<'_>,
    id: FunctionId,
    visiting: &mut BTreeSet<FunctionId>,
) -> Result<Properties> {
    if !visiting.insert(id) {
        return Ok(Properties::new());
    }
    let mut frame = Frame::default();
    for inst in file.function(id)?.instructions()? {
        let value = frame.value(file, &inst, visiting)?;
        frame.effect(&inst);
        frame.branched |= inst.operands().any(|(o, _)| o.kind.is_address());
        for written in inst.written_registers() {
            frame.registers.remove(&written);
        }
        if let Some(value) = value {
            frame.registers.insert(inst.values[0] as u32, value);
        }
    }
    visiting.remove(&id);
    Ok(frame.result.unwrap_or_default())
}

/// What one function's analysis knows at an instruction.
#[derive(Default)]
struct Frame {
    registers: BTreeMap<u32, Value>,
    objects: Vec<Object>,
    result: Option<Properties>,
    branched: bool,
}

impl Frame {
    fn register(&self, inst: &Instruction, operand: usize) -> Option<Value> {
        self.registers.get(&(inst.values[operand] as u32)).cloned()
    }

    fn object(&mut self, object: Object) -> Value {
        self.objects.push(object);
        Value::Object(self.objects.len() - 1)
    }

    /// The value `inst` writes to its first register, when the analysis follows it.
    fn value(
        &mut self,
        file: &HermesFile<'_>,
        inst: &Instruction,
        visiting: &mut BTreeSet<FunctionId>,
    ) -> Result<Option<Value>> {
        let v = &inst.values;
        Ok(match inst.op {
            Op::NewObject => Some(self.object(Object::default())),
            Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => {
                let slots = shape_keys(file, v[1] as u32)?
                    .iter()
                    .map(LiteralValue::string)
                    .collect::<Result<Vec<_>>>()?;
                let properties = slots.iter().flatten().map(|&key| (key, None)).collect();
                Some(self.object(Object { slots, properties }))
            }
            Op::NewArray => Some(Value::Strings(Vec::new())),
            Op::NewArrayWithBuffer | Op::NewArrayWithBufferLong => {
                read_literals(file.section(Section::Values), v[3] as usize, v[2] as usize)?
                    .iter()
                    .map(|literal| {
                        literal
                            .string()?
                            .map(|id| file.string(id)?.text())
                            .transpose()
                    })
                    .collect::<Result<Option<Vec<_>>>>()?
                    .map(Value::Strings)
            }
            Op::CreateClosure | Op::CreateClosureLongIndex => {
                Some(Value::Function(FunctionId(v[2] as u32)))
            }
            Op::Mov | Op::MovLong => self.register(inst, 1),
            Op::Call1 | Op::Call2 | Op::Call3 | Op::Call4 => match self.register(inst, 1) {
                Some(Value::Function(callee)) => {
                    let properties = returned(file, callee, visiting)?;
                    Some(self.object(Object {
                        slots: Vec::new(),
                        properties,
                    }))
                }
                _ => None,
            },
            _ => None,
        })
    }

    /// Records what `inst` assigns to a known object, or returns.
    fn effect(&mut self, inst: &Instruction) {
        let v = &inst.values;
        match inst.op {
            Op::PutOwnBySlotIdx | Op::PutOwnBySlotIdxLong => {
                if let Some(Value::Object(object)) = self.register(inst, 0) {
                    let value = self.register(inst, 1);
                    let object = &mut self.objects[object];
                    if let Some(&Some(key)) = object.slots.get(v[2] as usize) {
                        object.properties.insert(key, value);
                    }
                }
            }
            Op::PutByIdLoose
            | Op::PutByIdStrict
            | Op::PutByIdLooseLong
            | Op::PutByIdStrictLong
            | Op::DefineOwnById
            | Op::DefineOwnByIdLong
                if !self.branched =>
            {
                if let Some(Value::Object(object)) = self.register(inst, 0) {
                    let value = self.register(inst, 1);
                    self.objects[object]
                        .properties
                        .insert(StringId(v[3] as u32), value);
                }
            }
            Op::Ret => {
                let properties: Properties = match self.register(inst, 0) {
                    Some(Value::Object(object)) => self.objects[object]
                        .properties
                        .iter()
                        .map(|(&key, value)| {
                            let value = value
                                .clone()
                                .filter(|value| !matches!(value, Value::Object(_)));
                            (key, value)
                        })
                        .collect(),
                    _ => Properties::new(),
                };
                if let Some(result) = &mut self.result {
                    result.retain(|key, _| properties.contains_key(key));
                } else {
                    self.result = Some(properties);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{BUNDLED, HOST, LITERALS, function, image};
    use crate::{Code, Editor, FunctionIndex, HermesError, HookKind, Operand, SlotId, Statement};

    fn calling(module: crate::ModuleId, export: &str) -> Code {
        Code {
            blocks: vec![vec![Statement::Call {
                into: SlotId(0),
                module,
                export: export.into(),
                arguments: vec![Operand::Param(0)],
            }]],
        }
    }

    #[test]
    fn hooks_reach_the_exports_link_returns() {
        let host = image(HOST);
        let index = FunctionIndex::build(&host.file()).expect("host indexes");
        let greet = function(&host, &index, "greet");
        let modules: [(&[u8], &[&str]); 2] = [
            (LITERALS, &["rows", "render"]),
            (BUNDLED, &["isEnabled", "withProperty"]),
        ];
        for (bytes, exports) in modules {
            let module = image(bytes);
            let mut editor = Editor::new(host.file());
            let id = editor
                .link(&index, &module.file(), &[])
                .expect("module links");
            for export in exports {
                editor
                    .hook(&index, greet, HookKind::Before, &calling(id, export))
                    .unwrap_or_else(|error| panic!("{export} is exported: {error}"));
            }
            assert!(matches!(
                editor.hook(&index, greet, HookKind::Before, &calling(id, "missing")),
                Err(HermesError::Unsupported(_))
            ));
        }
    }
}
