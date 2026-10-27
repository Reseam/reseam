// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use crate::FunctionId;
use crate::dataflow::straight_line;
use crate::error::Result;
use crate::file::HermesFile;
use crate::literal::{LiteralValue, shape_keys};
use crate::opcode::Op;
use crate::string::StringId;

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

pub(super) fn shape(file: &HermesFile<'_>, factory: FunctionId) -> Result<ModuleShape> {
    let mut objects = Vec::<BTreeSet<StringId>>::new();
    let mut exports = BTreeSet::new();
    let mut replaced = None;
    let mut es_module = false;
    let code = file.function(factory)?.instructions()?;
    straight_line(&code, |inst, registers| {
        let v = &inst.values;
        let register = |operand: usize| registers.get(&(v[operand] as u32)).copied();
        let assigned = inst.stored_property().or_else(|| match inst.op {
            Op::DefineOwnGetterSetterByVal
            | Op::PutByValLoose
            | Op::PutByValStrict
            | Op::DefineOwnByVal => match register(1) {
                Some(Shaped::Text(key)) => Some(key),
                _ => None,
            },
            _ => None,
        });
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
        Ok(match inst.op {
            Op::LoadParam if v[1] == MODULE_PARAMETER => Some(Shaped::Module),
            Op::LoadParam if v[1] == EXPORTS_PARAMETER => Some(Shaped::Exports),
            Op::NewObject => {
                objects.push(BTreeSet::new());
                Some(Shaped::Object(objects.len() - 1))
            }
            Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => {
                objects.push(
                    shape_keys(file, v[1] as u32)?
                        .iter()
                        .filter_map(LiteralValue::string)
                        .collect(),
                );
                Some(Shaped::Object(objects.len() - 1))
            }
            Op::LoadConstString | Op::LoadConstStringLongIndex => {
                Some(Shaped::Text(StringId(v[1] as u32)))
            }
            Op::Mov | Op::MovLong => register(1),
            _ => None,
        })
    })?;
    Ok(ModuleShape {
        es_module,
        keys: replaced.map_or(exports, |object| std::mem::take(&mut objects[object])),
    })
}
