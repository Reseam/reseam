// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::Arc;

use boltffi::export;
use reseam_dex::{
    self as dex, AccessFlags, BlockOperand, BlockRegister, BlockStep, CodeItem, DexFile,
    IncomingBranches, Instruction as DexInsn, RegisterKind, TypeIdx,
};

mod returns;

use super::logged;
use super::mutation::{edit_mapping, growth_mapping};
use super::registers::{incoming_types, incoming_words};
use crate::bridge::dex::convert::kotlin_to_dex;
use crate::bridge::dex::link::link_instructions;
use crate::bridge::dex::types::{
    CodeEdit, CodeOperand, CodePlacement, CodeRegister, CodeStep, Instruction,
};
use crate::bridge::handles::with_method_mut;
use crate::context::method_mut;

/// Places code written over temporaries into method `m` and returns the edits that made, in
/// order. `protected` names registers the code must leave alone although nothing reads them yet.
#[export]
pub fn emit_code(
    m: u32,
    placement: CodePlacement,
    steps: Vec<CodeStep>,
    protected: Vec<u16>,
) -> Result<Vec<CodeEdit>, String> {
    let templates: Vec<Instruction> = steps
        .iter()
        .filter_map(|step| match step {
            CodeStep::Instruction(instruction) => Some(instruction.template.clone()),
            CodeStep::Label(_) => None,
        })
        .collect();
    link_instructions(&templates);
    let outcome = with_method_mut(m, |dex, loc| {
        let templates = logged(
            "convert emitted code",
            templates
                .iter()
                .map(|template| kotlin_to_dex(template, dex, Some(loc.dex_idx)))
                .collect::<dex::Result<Vec<_>>>(),
        )?;
        let method = logged("access method", method_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} is missing"))?;
        let (id, flags) = (method.method, method.access_flags);
        let mut body = method.code.take().map(Arc::unwrap_or_clone);
        let code = Code {
            dex,
            steps: &steps,
            templates,
            method: id,
            flags,
            incoming: incoming_types(dex, id, flags),
            protected,
        };
        let edits = code.apply(&mut body, placement);
        let method = logged("restore method", method_mut(dex, loc))?
            .expect("emitting code leaves the method slot in place");
        method.code = body.map(Arc::new);
        if edits.is_ok() && matches!(placement, CodePlacement::Body) {
            method
                .access_flags
                .remove(AccessFlags::NATIVE | AccessFlags::ABSTRACT);
        }
        edits.map(Some)
    });
    crate::bridge::run_state::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not take emitted code"))
}

fn existing(body: &mut Option<CodeItem>) -> Result<&mut CodeItem, String> {
    body.as_mut()
        .ok_or_else(|| "the method has no code".to_owned())
}

/// Emitted code, its instructions converted into the method's DEX.
struct Code<'a> {
    dex: &'a DexFile,
    steps: &'a [CodeStep],
    templates: Vec<DexInsn>,
    method: dex::MethodIdx,
    flags: AccessFlags,
    incoming: Vec<TypeIdx>,
    protected: Vec<u16>,
}

/// Where the parameters and the returned value an operand names are for one placement.
enum Frame<'a> {
    Incoming {
        base: u16,
    },
    Copies {
        copies: &'a BTreeMap<u16, u16>,
        returned: Option<u16>,
    },
}

impl Frame<'_> {
    fn register(&self, register: CodeRegister) -> Result<BlockRegister, String> {
        Ok(BlockRegister::Fixed(match (register, self) {
            (CodeRegister::Temporary(id), _) => return Ok(BlockRegister::Temporary(id)),
            (CodeRegister::Fixed(register), _) => register,
            (CodeRegister::Parameter(offset), Self::Incoming { base }) => base + offset,
            (CodeRegister::Parameter(offset), Self::Copies { copies, .. }) => copies[&offset],
            (CodeRegister::Returned, Self::Copies { returned, .. }) => {
                returned.ok_or("the return here returns no value")?
            }
            (CodeRegister::Returned, Self::Incoming { .. }) => {
                return Err("only code before a return reads the returned value".into());
            }
        }))
    }
}

impl Code<'_> {
    /// Places the code into `body`, or makes it the body.
    fn apply(
        &self,
        body: &mut Option<CodeItem>,
        placement: CodePlacement,
    ) -> Result<Vec<CodeEdit>, String> {
        match placement {
            CodePlacement::Before(index) => {
                self.insert(existing(body)?, index, IncomingBranches::Skip)
            }
            CodePlacement::OnEveryPath(index) => {
                self.insert(existing(body)?, index, IncomingBranches::Enter)
            }
            CodePlacement::BeforeReturns => self.before_returns(existing(body)?),
            CodePlacement::Body => {
                let dex = self.dex;
                let proto = dex.proto_descriptor(&dex.proto(dex.method_id(self.method).proto));
                let incoming = logged("replacement frame", incoming_words(&proto, self.flags))?;
                let mut replacement = logged(
                    "replacement frame",
                    CodeItem::new(incoming, incoming, 0, Vec::new()),
                )?;
                let block = self.block(&Frame::Incoming { base: 0 })?;
                self.place(&mut replacement, 0, IncomingBranches::Skip, &block, &[])?;
                *body = Some(replacement);
                Ok(Vec::new())
            }
        }
    }

    fn insert(
        &self,
        body: &mut CodeItem,
        index: u32,
        branches: IncomingBranches,
    ) -> Result<Vec<CodeEdit>, String> {
        let base = body.registers_size() - body.ins_size();
        let block = self.block(&Frame::Incoming { base })?;
        self.place(body, index as usize, branches, &block, &self.protected)
    }

    fn place(
        &self,
        body: &mut CodeItem,
        at: usize,
        branches: IncomingBranches,
        block: &[BlockStep],
        protected: &[u16],
    ) -> Result<Vec<CodeEdit>, String> {
        let base = body.registers_size() - body.ins_size();
        let inserted = logged(
            "emit code",
            dex::insert_block(
                body,
                at,
                block,
                branches,
                &self.incoming,
                protected,
                self.dex,
            ),
        )?;
        let mut edits = Vec::new();
        if let Some(growth) = inserted.growth {
            edits.push(CodeEdit::Grow(growth_mapping(
                base,
                growth.registers,
                growth.map,
            )?));
        }
        edits.push(CodeEdit::Insert(edit_mapping(inserted.insertion)?));
        Ok(edits)
    }

    /// The steps as a block, with parameters and the returned value where `frame` has them.
    fn block(&self, frame: &Frame<'_>) -> Result<Vec<BlockStep>, String> {
        let mut templates = self.templates.iter();
        self.steps
            .iter()
            .map(|step| match step {
                CodeStep::Label(label) => Ok(BlockStep::Label(*label)),
                CodeStep::Instruction(instruction) => Ok(BlockStep::Instruction {
                    instruction: templates
                        .next()
                        .expect("each instruction has its template")
                        .clone(),
                    operands: instruction
                        .operands
                        .iter()
                        .map(|operand| {
                            let register = frame.register(operand.register)?;
                            Ok(BlockOperand {
                                register,
                                kind: kind(operand),
                            })
                        })
                        .collect::<Result<_, String>>()?,
                    target: instruction.target,
                }),
            })
            .collect()
    }

    fn operands(&self) -> impl Iterator<Item = &CodeOperand> {
        self.steps
            .iter()
            .filter_map(|step| match step {
                CodeStep::Instruction(instruction) => Some(&instruction.operands),
                CodeStep::Label(_) => None,
            })
            .flatten()
    }
}

fn kind(operand: &CodeOperand) -> RegisterKind {
    RegisterKind::of(&operand.type_descriptor)
}
