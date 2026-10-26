// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::FunctionId;
use crate::dataflow::{Registers, register_states};
use crate::error::{Result, Unsupported, unsupported};
use crate::function::Function;
use crate::opcode::{Instruction, Op};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Value {
    Undefined,
    Environment(u16),
}

#[derive(Clone)]
pub(super) struct ClosureSite {
    pub target: FunctionId,
    pub instruction: usize,
    pub environment: Option<Value>,
    pub offset: u32,
}

/// Tracks which registers hold `undefined` or a known environment depth and reports what each
/// closure captures.
pub(super) fn analyse(
    function: &Function<'_>,
    code: &[Instruction],
    enclosing: u16,
) -> Result<Vec<ClosureSite>> {
    let states = register_states(function, code, |inst, before: &Registers<Value>| {
        let offset = inst.offset.expect("analysed instructions are decoded");
        let v = &inst.values;
        let shifted = |value: Value, delta: i32| -> Result<Value> {
            match value {
                Value::Environment(depth) => {
                    let depth = i32::from(depth) + delta;
                    let depth = u16::try_from(depth).map_err(|_| {
                        unsupported(function.id, Unsupported::EscapedEnvironment(offset))
                    })?;
                    Ok(Value::Environment(depth))
                }
                Value::Undefined => Err(unsupported(
                    function.id,
                    Unsupported::UnprovenEnvironment(offset),
                )),
            }
        };
        Ok(match inst.op {
            Op::LoadConstUndefined => Some(Value::Undefined),
            Op::Mov | Op::MovLong => before.get(&(v[1] as u32)).copied(),
            Op::GetParentEnvironment => {
                Some(shifted(Value::Environment(enclosing), -(v[1] as i32))?)
            }
            Op::CreateFunctionEnvironment => Some(shifted(Value::Environment(enclosing), 1)?),
            Op::CreateTopLevelEnvironment => Some(Value::Environment(1)),
            Op::CreateEnvironment => before
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, 1))
                .transpose()?,
            Op::GetEnvironment => before
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, -(v[2] as i32)))
                .transpose()?,
            _ => None,
        })
    })?;
    let mut sites = Vec::new();
    for (index, inst) in code.iter().enumerate() {
        if !matches!(inst.op, Op::CreateClosure | Op::CreateClosureLongIndex) {
            continue;
        }
        let Some(state) = states.get(&index) else {
            continue;
        };
        sites.push(ClosureSite {
            target: FunctionId(inst.values[2] as u32),
            instruction: index,
            environment: state.get(&(inst.values[1] as u32)).copied(),
            offset: inst.offset.expect("analysed instructions are decoded"),
        });
    }
    Ok(sites)
}
