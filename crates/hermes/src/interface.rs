// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::dataflow::{Registers, straight_line};
use crate::error::{HermesError, LinkError, Result};
use crate::layout::Section;
use crate::literal::{LiteralValue, read_literals, shape_keys};
use crate::opcode::{Instruction, Op};
use crate::string::{StringId, StringValue};
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
    straight_line(&file.function(id)?.instructions()?, |inst, registers| {
        let value = frame.value(file, inst, registers, visiting)?;
        frame.effect(inst, registers);
        frame.branched |= inst.jumps().next().is_some();
        Ok(value)
    })?;
    visiting.remove(&id);
    Ok(frame.result.unwrap_or_default())
}

/// The texts of an array literal, when every element is a string.
fn texts(file: &HermesFile<'_>, offset: usize, count: usize) -> Result<Option<Vec<String>>> {
    let mut texts = Vec::with_capacity(count);
    for literal in read_literals(file, Section::Values, offset, count)? {
        let value = literal.string().map(|id| file.string(id)).transpose()?;
        let Some(text) = value.and_then(StringValue::text) else {
            return Ok(None);
        };
        texts.push(text);
    }
    Ok(Some(texts))
}

/// What one function's analysis knows besides its registers.
#[derive(Default)]
struct Frame {
    objects: Vec<Object>,
    result: Option<Properties>,
    branched: bool,
}

fn register(registers: &Registers<Value>, inst: &Instruction, operand: usize) -> Option<Value> {
    registers.get(&(inst.values[operand] as u32)).cloned()
}

impl Frame {
    fn object(&mut self, object: Object) -> Value {
        self.objects.push(object);
        Value::Object(self.objects.len() - 1)
    }

    /// The value `inst` writes to its first register, when the analysis follows it.
    fn value(
        &mut self,
        file: &HermesFile<'_>,
        inst: &Instruction,
        registers: &Registers<Value>,
        visiting: &mut BTreeSet<FunctionId>,
    ) -> Result<Option<Value>> {
        let v = &inst.values;
        Ok(match inst.op {
            Op::NewObject => Some(self.object(Object::default())),
            Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => {
                let slots: Vec<_> = shape_keys(file, v[1] as u32)?
                    .iter()
                    .map(LiteralValue::string)
                    .collect();
                let properties = slots.iter().flatten().map(|&key| (key, None)).collect();
                Some(self.object(Object { slots, properties }))
            }
            Op::NewArray => Some(Value::Strings(Vec::new())),
            Op::NewArrayWithBuffer | Op::NewArrayWithBufferLong => {
                texts(file, v[3] as usize, v[2] as usize)?.map(Value::Strings)
            }
            Op::CreateClosure | Op::CreateClosureLongIndex => {
                Some(Value::Function(FunctionId(v[2] as u32)))
            }
            Op::Mov | Op::MovLong => register(registers, inst, 1),
            Op::Call1 | Op::Call2 | Op::Call3 | Op::Call4 => match register(registers, inst, 1) {
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
    fn effect(&mut self, inst: &Instruction, registers: &Registers<Value>) {
        let object = || match register(registers, inst, 0) {
            Some(Value::Object(object)) => Some(object),
            _ => None,
        };
        if let Some(key) = inst.stored_property() {
            if !self.branched
                && let Some(object) = object()
            {
                self.objects[object]
                    .properties
                    .insert(key, register(registers, inst, 1));
            }
            return;
        }
        match inst.op {
            Op::PutOwnBySlotIdx | Op::PutOwnBySlotIdxLong => {
                if let Some(object) = object() {
                    let object = &mut self.objects[object];
                    if let Some(&Some(key)) = object.slots.get(inst.values[2] as usize) {
                        object.properties.insert(key, register(registers, inst, 1));
                    }
                }
            }
            Op::Ret => {
                let properties: Properties = object().map_or_else(Properties::new, |object| {
                    self.objects[object]
                        .properties
                        .iter()
                        .map(|(&key, value)| {
                            let value = value
                                .clone()
                                .filter(|value| !matches!(value, Value::Object(_)));
                            (key, value)
                        })
                        .collect()
                });
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
    use crate::error::Unsupported;
    use crate::fixtures::{BUNDLED, LITERALS, function, host, image};
    use crate::{Code, HermesError, HookKind, Operand, SlotId, Statement};

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
        let greet = function(&host(), "greet");
        let modules: [(&[u8], &[&str]); 2] = [
            (LITERALS, &["rows", "render"]),
            (BUNDLED, &["isEnabled", "withProperty"]),
        ];
        for (bytes, exports) in modules {
            let module = image(bytes);
            let mut editor = host();
            let id = editor.link(&module.file(), &[]).expect("module links");
            for export in exports {
                editor
                    .hook(greet, HookKind::Before, &calling(id, export))
                    .unwrap_or_else(|error| panic!("{export} is exported: {error}"));
            }
            assert!(matches!(
                editor.hook(greet, HookKind::Before, &calling(id, "missing")),
                Err(HermesError::Unsupported {
                    reason: Unsupported::Export(_),
                    ..
                })
            ));
        }
    }
}
