// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::Result;
use crate::instruction::Instruction;
use crate::instruction::stream::{RawInstruction, walk_instructions};
use crate::model::access_flags::AccessFlags;
use crate::model::class::MethodKind;
use crate::model::{FieldIdx, MethodIdx, StringIdx, TypeIdx};
use reseam_bytes::read_u32;

/// One instruction as a search sees it: the opcode and the pool reference or
/// literal it carries, from decoded IR or straight from code units.
#[derive(Clone, Copy)]
pub enum InstructionRef<'a> {
    Decoded(&'a Instruction),
    Raw { buf: &'a [u8], insn: RawInstruction },
}

impl InstructionRef<'_> {
    pub fn opcode(&self) -> Option<u16> {
        match self {
            Self::Decoded(insn) => insn.opcode(),
            Self::Raw { insn, .. } => insn.opcode(),
        }
    }

    pub fn method_ref(&self) -> Option<MethodIdx> {
        match self {
            Self::Decoded(insn) => insn.method_ref(),
            Self::Raw { buf, insn } => insn.method_ref(buf),
        }
    }

    pub fn field_ref(&self) -> Option<FieldIdx> {
        match self {
            Self::Decoded(insn) => insn.field_ref(),
            Self::Raw { buf, insn } => insn.field_ref(buf),
        }
    }

    pub fn string_ref(&self) -> Option<StringIdx> {
        match self {
            Self::Decoded(insn) => insn.string_ref(),
            Self::Raw { buf, insn } => insn.string_ref(buf),
        }
    }

    pub fn type_ref(&self) -> Option<TypeIdx> {
        match self {
            Self::Decoded(insn) => insn.type_ref(),
            Self::Raw { buf, insn } => insn.type_ref(buf),
        }
    }

    pub fn literal(&self) -> Option<i64> {
        match self {
            Self::Decoded(insn) => insn.literal(),
            Self::Raw { buf, insn } => insn.literal(buf),
        }
    }
}

/// A method encountered during a scan. Cheap checks (name, flags, prototype)
/// come from the member list; instructions are walked only when asked for.
pub struct MethodView<'a> {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    /// Index of the defining class within [`DexFile::classes`].
    pub class_idx: usize,
    pub class_type: TypeIdx,
    /// Position within the class's direct- or virtual-method list.
    pub method_pos: usize,
    pub kind: MethodKind,
    pub(super) code: Code<'a>,
}

pub(super) enum Code<'a> {
    None,
    Resolved(&'a [Instruction]),
    Raw { buf: &'a [u8], code_off: u32 },
}

impl MethodView<'_> {
    pub fn has_code(&self) -> bool {
        !matches!(self.code, Code::None)
    }

    /// Visits each instruction in order, stopping when `visit` returns `false`.
    pub fn for_each_instruction(
        &self,
        mut visit: impl FnMut(InstructionRef<'_>) -> bool,
    ) -> Result<()> {
        match &self.code {
            Code::None => Ok(()),
            Code::Resolved(instructions) => {
                for insn in *instructions {
                    if !visit(InstructionRef::Decoded(insn)) {
                        break;
                    }
                }
                Ok(())
            }
            Code::Raw { buf, code_off } => {
                let base = *code_off as usize;
                let insns_size = read_u32(buf, base + 12, "code item")? as usize;
                walk_instructions(buf, base + 16, insns_size, |insn| {
                    visit(InstructionRef::Raw { buf, insn: *insn })
                })
            }
        }
    }

    /// Whether any instruction satisfies `pred`.
    pub fn any_instruction(
        &self,
        mut pred: impl FnMut(InstructionRef<'_>) -> bool,
    ) -> Result<bool> {
        let mut found = false;
        self.for_each_instruction(|insn| {
            found = pred(insn);
            !found
        })?;
        Ok(found)
    }

    /// The method's opcode sequence, into `out`.
    pub fn opcodes(&self, out: &mut Vec<Option<u16>>) -> Result<()> {
        out.clear();
        self.for_each_instruction(|insn| {
            out.push(insn.opcode());
            true
        })
    }

    pub fn hit(&self) -> MethodHit {
        MethodHit {
            class_idx: self.class_idx,
            class_type: self.class_type,
            method: self.method,
            method_pos: self.method_pos,
            kind: self.kind,
        }
    }
}

/// A located method match. Carries indices rather than borrows so it survives
/// both resolved and raw scanning and needs no pointer recovery afterwards.
#[derive(Debug, Clone)]
pub struct MethodHit {
    pub class_idx: usize,
    pub class_type: TypeIdx,
    pub method: MethodIdx,
    pub method_pos: usize,
    pub kind: MethodKind,
}

#[derive(Debug, Clone)]
pub struct InstructionHit {
    pub class_idx: usize,
    pub method_pos: usize,
    pub kind: MethodKind,
    pub insn_idx: usize,
}

pub struct InstructionSite<'a> {
    pub class_idx: usize,
    pub method_pos: usize,
    pub kind: MethodKind,
    pub insn_idx: usize,
    pub instruction: InstructionRef<'a>,
}
