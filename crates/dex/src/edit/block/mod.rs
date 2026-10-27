// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Code written over temporaries instead of registers. Inserting it chooses a register for
//! each temporary, reaches operands an encoding cannot name through scratch registers, lowers
//! calls, lays out branches, and grows the frame when the method has too few free registers.

mod allocate;
mod emit;
mod lifetimes;
mod planned;
mod program;

use allocate::{Frame, Unallocated};
use program::Program;

use super::body::IncomingBranches;
use super::grow::grow_registers;
use super::relocate::InstructionMap;
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::operands::RegisterKind;
use crate::model::code::CodeItem;
use crate::{DexFile, TypeIdx};

/// Where an operand of a block lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRegister {
    /// A value the block computes, in a register the insertion chooses.
    Temporary(u32),
    /// A register of the method, numbered as before the insertion.
    Fixed(u16),
}

#[derive(Debug, Clone, Copy)]
pub struct BlockOperand {
    pub register: BlockRegister,
    pub kind: RegisterKind,
}

pub enum BlockStep {
    /// Marks the next instruction, or the end of the block, as where branches to it go.
    Label(u32),
    /// An instruction whose register operands, in encoding order, are `operands`; for a call,
    /// they are its arguments, one per argument. Its registers and the width of its move or
    /// constant are chosen on insertion. A branch goes to the label `target`.
    Instruction {
        instruction: Instruction,
        operands: Vec<BlockOperand>,
        target: Option<u32>,
    },
}

/// Locals added below the incoming registers, and where that moved each instruction.
pub struct FrameGrowth {
    pub registers: u16,
    pub map: InstructionMap,
}

pub struct BlockInsertion {
    pub growth: Option<FrameGrowth>,
    pub insertion: InstructionMap,
}

/// Inserts `steps` before instruction `at`. Temporaries take registers no value live at `at`
/// occupies, other than the incoming registers, the `protected` ones and those the block names;
/// the frame grows when those run out. `incoming` types the method's parameters.
pub fn insert_block(
    code: &mut CodeItem,
    at: usize,
    steps: &[BlockStep],
    branches: IncomingBranches,
    incoming: &[TypeIdx],
    protected: &[u16],
    dex: &DexFile,
) -> Result<BlockInsertion> {
    let program = Program::new(steps, dex)?;
    let held: Vec<u16> = protected
        .iter()
        .copied()
        .chain(program.fixed_registers())
        .collect();
    let frame = Frame::new(code, at, &held)?;
    let mut growth = 0u16;
    let plan = loop {
        match frame.allocate(&program, growth) {
            Ok(plan) => break plan,
            Err(Unallocated::Short(words)) => {
                growth = growth
                    .checked_add(words)
                    .ok_or_else(|| invalid("code block", "frame exceeds 65535 registers"))?;
            }
            Err(Unallocated::Unreachable(error)) => return Err(error),
        }
    };
    let instructions = emit::emit(&program, &plan)?;
    let growth = if growth == 0 {
        None
    } else {
        Some(FrameGrowth {
            registers: growth,
            map: grow_registers(code, growth, incoming, &held, dex)?,
        })
    };
    let at = growth.as_ref().map_or(at, |growth| growth.map.starts()[at]);
    let insertion = code.insert(at, &instructions, branches)?;
    Ok(BlockInsertion { growth, insertion })
}
