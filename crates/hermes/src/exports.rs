// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::model::StringId;
use crate::opcode::Op;
use crate::{FunctionId, HermesFile, Result};

#[derive(Clone, Copy)]
enum Value {
    Object(usize),
    Function(FunctionId),
}

/// The properties a module's global function returns on its exports object:
/// those every return path assigns a closure to, before any branch.
pub(crate) fn exports(file: &HermesFile<'_>) -> Result<Vec<StringId>> {
    resolve(file, file.global_function(), &mut BTreeSet::new())
        .map(|fields| fields.into_iter().collect())
}

fn resolve(
    file: &HermesFile<'_>,
    id: FunctionId,
    visiting: &mut BTreeSet<FunctionId>,
) -> Result<BTreeSet<StringId>> {
    if !visiting.insert(id) {
        return Ok(BTreeSet::new());
    }
    let mut registers = BTreeMap::<u32, Value>::new();
    let mut objects = Vec::<BTreeSet<StringId>>::new();
    let mut returned: Option<BTreeSet<StringId>> = None;
    let mut branched = false;
    for inst in file.function(id)?.instructions()? {
        let v = &inst.values;
        let register = |operand: usize| registers.get(&(v[operand] as u32)).copied();
        let value = match inst.op {
            Op::NewObject => {
                objects.push(BTreeSet::new());
                Some(Value::Object(objects.len() - 1))
            }
            Op::CreateClosure | Op::CreateClosureLongIndex => {
                Some(Value::Function(FunctionId(v[2] as u32)))
            }
            Op::Mov | Op::MovLong => register(1),
            Op::Call1 | Op::Call2 | Op::Call3 | Op::Call4 => {
                if let Some(Value::Function(callee)) = register(1) {
                    objects.push(resolve(file, callee, visiting)?);
                    Some(Value::Object(objects.len() - 1))
                } else {
                    None
                }
            }
            _ => None,
        };
        if matches!(
            inst.op,
            Op::PutByIdLoose
                | Op::PutByIdStrict
                | Op::PutByIdLooseLong
                | Op::PutByIdStrictLong
                | Op::DefineOwnById
                | Op::DefineOwnByIdLong
        ) && let Some(Value::Object(object)) = register(0)
        {
            let key = StringId(v[3] as u32);
            if !branched && matches!(register(1), Some(Value::Function(_))) {
                objects[object].insert(key);
            } else {
                objects[object].remove(&key);
            }
        }
        if inst.op == Op::Ret {
            let fields = if let Some(Value::Object(object)) = register(0) {
                objects[object].clone()
            } else {
                BTreeSet::new()
            };
            if let Some(returned) = &mut returned {
                returned.retain(|key| fields.contains(key));
            } else {
                returned = Some(fields);
            }
        }
        branched |= inst.operands().any(|(o, _)| o.kind.is_address());
        for written in inst.written_registers() {
            registers.remove(&written);
        }
        if let Some(value) = value {
            registers.insert(v[0] as u32, value);
        }
    }
    visiting.remove(&id);
    Ok(returned.unwrap_or_default())
}
