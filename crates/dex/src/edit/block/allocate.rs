// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use fixedbitset::FixedBitSet;

use super::BlockRegister;
use super::lifetimes::Temporary;
use super::planned::Planned;
use super::program::Program;
use crate::edit::invoke::{Invocation, InvokeForm};
use crate::edit::liveness::RegisterLiveness;
use crate::error::{DexError, Result, invalid};
use crate::model::code::CodeItem;

/// The method a block goes into: its locals below `base`, those unavailable at the insertion
/// point, and its incoming registers from `base`, which growth moves up.
pub(super) struct Frame {
    base: u16,
    incoming: u16,
    unavailable: FixedBitSet,
}

pub(super) enum Unallocated {
    /// The frame needs this many more locals.
    Short(u16),
    Unreachable(DexError),
}

type Allocation<T> = std::result::Result<T, Unallocated>;

/// The register of each temporary, the scratch register staging each operand an encoding
/// cannot reach, and the scratch span each call copies its arguments to.
pub(super) struct Plan {
    base: u16,
    growth: u16,
    temporaries: Vec<Option<u16>>,
    pub(super) staging: Vec<Vec<Option<u16>>>,
    pub(super) spans: Vec<Option<u16>>,
}

impl Plan {
    pub(super) fn register(&self, register: BlockRegister) -> u16 {
        match register {
            BlockRegister::Temporary(id) => {
                self.temporaries[id as usize].expect("allocation gives every temporary a register")
            }
            BlockRegister::Fixed(register) if register >= self.base => register + self.growth,
            BlockRegister::Fixed(register) => register,
        }
    }

    /// Every register word the operands of `planned` name.
    fn operand_words<'p>(&'p self, planned: &'p Planned) -> impl Iterator<Item = u16> + 'p {
        planned.operands.iter().flat_map(|operand| {
            let register = self.register(operand.register);
            register..register + operand.kind.words()
        })
    }
}

impl Frame {
    /// `held` names registers, in the method's numbering, no temporary may take.
    pub(super) fn new(code: &CodeItem, at: usize, held: &[u16]) -> Result<Self> {
        let base = code
            .registers_size
            .checked_sub(code.ins_size)
            .ok_or_else(|| invalid("code block", "parameters exceed frame"))?;
        let liveness = RegisterLiveness::new(code);
        let mut unavailable = FixedBitSet::with_capacity(usize::from(base));
        unavailable.extend(
            (0..base)
                .filter(|&register| liveness.is_live(at, register))
                .chain(held.iter().copied().filter(|&register| register < base))
                .map(usize::from),
        );
        Ok(Self {
            base,
            incoming: code.ins_size,
            unavailable,
        })
    }

    /// Registers for `program` once the frame has `growth` more locals, or how many more it
    /// needs. Temporaries take the lowest registers their uses reach; an operand out of reach
    /// is staged through a free scratch register that is in reach.
    pub(super) fn allocate(&self, program: &Program, growth: u16) -> Allocation<Plan> {
        let locals = self.base + growth;
        if u32::from(locals) + u32::from(self.incoming) > u32::from(u16::MAX) {
            return Err(Unallocated::Unreachable(invalid(
                "code block",
                "frame exceeds 65535 registers",
            )));
        }
        let mut reserved = self.unavailable.clone();
        reserved.grow(usize::from(locals));
        let mut plan = Plan {
            base: self.base,
            growth,
            temporaries: vec![None; program.temporaries.len()],
            staging: Vec::with_capacity(program.instructions.len()),
            spans: Vec::with_capacity(program.instructions.len()),
        };
        let mut order: Vec<(usize, &Temporary)> = program
            .temporaries
            .iter()
            .enumerate()
            .filter_map(|(id, temporary)| Some((id, temporary.as_ref()?)))
            .collect();
        order.sort_by_key(|(_, temporary)| temporary.first);
        for (id, temporary) in order {
            let mut busy = reserved.clone();
            occupy_temporaries(&mut busy, program, &plan, temporary.first, temporary.last);
            let words = temporary.kind.words();
            let register = lowest_free(&busy, words, Some(temporary.max))
                .or_else(|| lowest_free(&busy, words, None))
                .ok_or(Unallocated::Short(words))?;
            plan.temporaries[id] = Some(register);
        }
        for (index, planned) in program.instructions.iter().enumerate() {
            let mut busy = reserved.clone();
            occupy_temporaries(&mut busy, program, &plan, 2 * index, 2 * index + 1);
            busy.extend(
                plan.operand_words(planned)
                    .filter(|&register| register < locals)
                    .map(usize::from),
            );
            let staging = stage(&mut busy, planned, &plan, locals, index)?;
            let span = span(&busy, planned, &plan)?;
            plan.staging.push(staging);
            plan.spans.push(span);
        }
        Ok(plan)
    }
}

/// A scratch register for each operand of `planned` its encoding cannot reach.
fn stage(
    busy: &mut FixedBitSet,
    planned: &Planned,
    plan: &Plan,
    locals: u16,
    index: usize,
) -> Allocation<Vec<Option<u16>>> {
    let mut staging = Vec::with_capacity(planned.uses.len());
    for (position, (operand, operand_use)) in planned.operands.iter().zip(&planned.uses).enumerate()
    {
        if planned.arguments.is_some() || plan.register(operand.register) <= operand_use.max {
            staging.push(None);
            continue;
        }
        let words = operand.kind.words();
        let Some(scratch) = lowest_free(busy, words, Some(operand_use.max)) else {
            // Growth adds locals from `locals` up, which helps only when they are in reach.
            let reachable = u32::from(locals) + u32::from(words) <= u32::from(operand_use.max) + 1;
            return Err(if reachable {
                Unallocated::Short(words)
            } else {
                Unallocated::Unreachable(invalid(
                    "code block",
                    format!(
                        "operand {position} of instruction {index} needs a free register at or below v{}",
                        operand_use.max
                    ),
                ))
            });
        };
        busy.extend((scratch..scratch + words).map(usize::from));
        staging.push(Some(scratch));
    }
    Ok(staging)
}

/// The first of the consecutive registers a call copies its arguments to when no encoding
/// reaches them where they are.
fn span(busy: &FixedBitSet, planned: &Planned, plan: &Plan) -> Allocation<Option<u16>> {
    let Some(kinds) = &planned.arguments else {
        return Ok(None);
    };
    let registers: Vec<u16> = plan.operand_words(planned).collect();
    let words = Invocation::new(InvokeForm::Compact, &registers, kinds)
        .map_err(Unallocated::Unreachable)?
        .scratch_words();
    if words == 0 {
        return Ok(None);
    }
    lowest_free(busy, words, None)
        .map(Some)
        .ok_or(Unallocated::Short(words))
}

/// Marks the registers of the temporaries holding a value anywhere from `first` to `last`.
fn occupy_temporaries(
    busy: &mut FixedBitSet,
    program: &Program,
    plan: &Plan,
    first: usize,
    last: usize,
) {
    for (temporary, register) in program.temporaries.iter().zip(&plan.temporaries) {
        if let (Some(temporary), Some(register)) = (temporary, register)
            && temporary.overlaps(first, last)
        {
            busy.extend((*register..*register + temporary.kind.words()).map(usize::from));
        }
    }
}

/// The lowest run of `words` registers free in `busy`, ending at or below `max`.
fn lowest_free(busy: &FixedBitSet, words: u16, max: Option<u16>) -> Option<u16> {
    let end = busy.len();
    (0..end.saturating_sub(usize::from(words) - 1))
        .find(|&start| {
            max.is_none_or(|max| start + usize::from(words) - 1 <= usize::from(max))
                && (start..start + usize::from(words)).all(|register| !busy.contains(register))
        })
        .map(|start| start as u16)
}
