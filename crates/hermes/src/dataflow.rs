// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, VecDeque};

use crate::error::{Result, invalid};
use crate::function::Function;
use crate::opcode::{Instruction, Op};
use crate::switch::switch_table;

/// Values known to be held by registers, by register number.
pub(crate) type Registers<V> = BTreeMap<u32, V>;

/// The register values known on entry to each reachable instruction, by instruction index.
/// `transfer` gives the value an instruction leaves in its first operand register from the values
/// known before it, or `None` when that value is unknown; every other register the instruction
/// writes becomes unknown. Paths merge by keeping only the values they agree on. An exception
/// handler starts from the state before the throwing instruction, less the registers it writes.
pub(crate) fn register_states<V: Copy + Eq>(
    function: &Function<'_>,
    code: &[Instruction],
    mut transfer: impl FnMut(&Instruction, &Registers<V>) -> Result<Option<V>>,
) -> Result<BTreeMap<usize, Registers<V>>> {
    if code.is_empty() {
        return Ok(BTreeMap::new());
    }
    let indices: BTreeMap<_, _> = code
        .iter()
        .enumerate()
        .map(|(i, inst)| (inst.offset.expect("analysed instructions are decoded"), i))
        .collect();
    let handlers = function.exception_handlers()?;
    let mut states = BTreeMap::from([(0_usize, Registers::new())]);
    let mut work = VecDeque::from([0_usize]);
    while let Some(index) = work.pop_front() {
        let before = states[&index].clone();
        let inst = &code[index];
        let offset = inst.offset.expect("analysed instructions are decoded");
        let value = transfer(inst, &before)?;
        let mut thrown = before;
        for register in inst.written_registers() {
            thrown.remove(&register);
        }
        let mut after = thrown.clone();
        if let Some(value) = value {
            after.insert(inst.values[0] as u32, value);
        }
        for next in successors(function, code, index, &indices)? {
            flow(&mut states, &mut work, next, &after);
        }
        for handler in handlers.iter().filter(|h| h.covers(offset)) {
            let next = *indices.get(&handler.target).ok_or_else(|| {
                invalid(
                    function.header.offset as usize + handler.target as usize,
                    "exception target is not an instruction",
                )
            })?;
            flow(&mut states, &mut work, next, &thrown);
        }
    }
    Ok(states)
}

/// Follows register values through `code` in order, ignoring control flow. `step` sees each
/// instruction with the values known before it and gives the value it leaves in its first operand
/// register; every other register the instruction writes becomes unknown.
pub(crate) fn straight_line<V>(
    code: &[Instruction],
    mut step: impl FnMut(&Instruction, &Registers<V>) -> Result<Option<V>>,
) -> Result<()> {
    let mut registers = Registers::new();
    for inst in code {
        let value = step(inst, &registers)?;
        for written in inst.written_registers() {
            registers.remove(&written);
        }
        if let Some(value) = value {
            registers.insert(inst.values[0] as u32, value);
        }
    }
    Ok(())
}

/// Merges `state` into the entry state of `next`, queueing it when that changes.
fn flow<V: Copy + Eq>(
    states: &mut BTreeMap<usize, Registers<V>>,
    work: &mut VecDeque<usize>,
    next: usize,
    state: &Registers<V>,
) {
    if let Some(existing) = states.get_mut(&next) {
        let before = existing.len();
        existing.retain(|reg, value| state.get(reg) == Some(value));
        if existing.len() != before {
            work.push_back(next);
        }
    } else {
        states.insert(next, state.clone());
        work.push_back(next);
    }
}

fn successors(
    function: &Function<'_>,
    code: &[Instruction],
    index: usize,
    indices: &BTreeMap<u32, usize>,
) -> Result<Vec<usize>> {
    let inst = &code[index];
    let origin = i64::from(inst.offset.expect("analysed instructions are decoded"));
    let mut result = Vec::new();
    let start = function.header.offset as usize;
    let mut add = |relative: i64| -> Result<()> {
        let target = u32::try_from(origin + relative)
            .ok()
            .and_then(|target| indices.get(&target))
            .ok_or_else(|| invalid(start + origin as usize, "branch outside instructions"))?;
        result.push(*target);
        Ok(())
    };
    for (_, relative) in inst.jumps() {
        add(relative)?;
    }
    if let Some(table) = switch_table(function, inst)? {
        for (_, target) in table.entries() {
            add(i64::from(target))?;
        }
    }
    if !matches!(
        inst.op,
        Op::Ret
            | Op::Throw
            | Op::Unreachable
            | Op::Jmp
            | Op::JmpLong
            | Op::UIntSwitchImm
            | Op::StringSwitchImm
    ) && index + 1 < code.len()
    {
        result.push(index + 1);
    }
    Ok(result)
}
