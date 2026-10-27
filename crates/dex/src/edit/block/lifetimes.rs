// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use fixedbitset::FixedBitSet;

use super::BlockRegister;
use super::planned::Planned;
use crate::error::{Result, invalid};
use crate::instruction::operands::{Access, RegisterKind};

/// A temporary's kind, the highest register every use reaches, and the positions it holds a
/// value over: `2 * i` on entry to instruction `i`, `2 * i + 1` on its exit.
pub(super) struct Temporary {
    pub(super) kind: RegisterKind,
    pub(super) max: u16,
    pub(super) first: usize,
    pub(super) last: usize,
}

impl Temporary {
    pub(super) fn overlaps(&self, first: usize, last: usize) -> bool {
        self.first <= last && first <= self.last
    }
}

/// Each temporary's kind and the span over which it holds a value, from its liveness across
/// the block's branches.
pub(super) fn temporaries(instructions: &[Planned]) -> Result<Vec<Option<Temporary>>> {
    let count = instructions
        .iter()
        .flat_map(|planned| planned.temporaries(|_| true))
        .max()
        .map_or(0, |id| id + 1);
    let reads: Vec<FixedBitSet> = instructions
        .iter()
        .map(|planned| set(count, planned.temporaries(|access| access != Access::Write)))
        .collect();
    let writes: Vec<FixedBitSet> = instructions
        .iter()
        .map(|planned| set(count, planned.temporaries(Access::writes_value)))
        .collect();
    let mut live_in = vec![FixedBitSet::with_capacity(count); instructions.len() + 1];
    let mut live_out = vec![FixedBitSet::with_capacity(count); instructions.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for (index, planned) in instructions.iter().enumerate().rev() {
            let mut out = FixedBitSet::with_capacity(count);
            for next in planned.successors(index) {
                out.union_with(&live_in[next]);
            }
            let mut input = out.clone();
            input.difference_with(&writes[index]);
            input.union_with(&reads[index]);
            changed |= input != live_in[index] || out != live_out[index];
            live_in[index] = input;
            live_out[index] = out;
        }
    }
    if live_in.first().is_some_and(|live| !live.is_clear()) {
        return Err(invalid(
            "code block",
            "a temporary is read before it is written",
        ));
    }
    let mut temporaries: Vec<Option<Temporary>> = (0..count).map(|_| None).collect();
    for planned in instructions {
        for (operand, operand_use) in planned.operands.iter().zip(&planned.uses) {
            let BlockRegister::Temporary(id) = operand.register else {
                continue;
            };
            let temporary = temporaries[id as usize].get_or_insert(Temporary {
                kind: operand.kind,
                max: u16::MAX,
                first: usize::MAX,
                last: 0,
            });
            if temporary.kind.words() != operand.kind.words() {
                return Err(invalid("code block", "a temporary changes its width"));
            }
            temporary.max = temporary.max.min(operand_use.max);
        }
    }
    for index in 0..instructions.len() {
        let entry = live_in[index]
            .union(&reads[index])
            .map(|id| (id, 2 * index));
        let exit = live_out[index]
            .union(&writes[index])
            .map(|id| (id, 2 * index + 1));
        for (id, position) in entry.chain(exit) {
            let temporary = temporaries[id]
                .as_mut()
                .expect("a live temporary is an operand of some instruction");
            temporary.first = temporary.first.min(position);
            temporary.last = temporary.last.max(position);
        }
    }
    Ok(temporaries)
}

fn set(count: usize, members: impl Iterator<Item = usize>) -> FixedBitSet {
    let mut set = FixedBitSet::with_capacity(count);
    set.extend(members);
    set
}
