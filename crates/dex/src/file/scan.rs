// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod inspection;

pub use inspection::{MethodSummary, summarize_resident};

use std::ops::ControlFlow;

use rayon::prelude::*;

use super::pattern::InstructionPattern;
use super::ref_filter::{ClassFilter, RefFilter};
use super::{DexFile, Fingerprint, RefQuery};
use crate::error::Result;
pub use crate::read::class::MemberCounts;
use crate::read::class::{ClassDataCursor, ClassSkeleton, MethodHeader, read_class_skeleton_at};
use crate::read::code::{RawInstruction, count_instructions, read_code_item, walk_instructions};
use crate::read::{read_u16, read_u32};
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, EncodedField, EncodedMethod};
use crate::types::header::ParseOptions;
use crate::types::instruction::Instruction;
use crate::types::{FieldIdx, MethodIdx, StringIdx, TypeIdx};

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
    pub kind: crate::types::class::MethodKind,
    code: Code<'a>,
}

enum Code<'a> {
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
                let insns_size = read_u32(buf, base + 12)? as usize;
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
    pub kind: crate::types::class::MethodKind,
}

#[derive(Debug, Clone)]
pub struct InstructionHit {
    pub class_idx: usize,
    pub method_pos: usize,
    pub kind: crate::types::class::MethodKind,
    pub insn_idx: usize,
}

pub struct InstructionSite<'a> {
    pub class_idx: usize,
    pub method_pos: usize,
    pub kind: crate::types::class::MethodKind,
    pub insn_idx: usize,
    pub instruction: InstructionRef<'a>,
}

impl DexFile {
    /// Scans methods across all classes, returning the first `Some`. Sequential
    /// with early exit.
    pub fn scan_methods_find<T>(
        &self,
        query: &RefQuery,
        mut f: impl FnMut(&MethodView<'_>) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let filter = self.filter_for(query)?;
        for class_idx in 0..self.classes.len() {
            let flow = self.scan_class(class_idx, filter, query, &mut |view| {
                Ok(match f(view)? {
                    Some(value) => ControlFlow::Break(value),
                    None => ControlFlow::Continue(()),
                })
            })?;
            if let ControlFlow::Break(value) = flow {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Scans every method across all classes in parallel, collecting each `Some`
    /// in class-then-method order. Only methods the reference filter admits
    /// for `query` are visited.
    pub fn scan_methods_collect<T: Send>(
        &self,
        query: &RefQuery,
        f: impl Fn(&MethodView<'_>) -> Result<Option<T>> + Sync,
    ) -> Result<Vec<T>> {
        let filter = self.filter_for(query)?;
        let per_class: Vec<Vec<T>> = (0..self.classes.len())
            .into_par_iter()
            .map(|class_idx| {
                let mut hits = Vec::new();
                let _: ControlFlow<()> =
                    self.scan_class(class_idx, filter, query, &mut |view| {
                        if let Some(value) = f(view)? {
                            hits.push(value);
                        }
                        Ok(ControlFlow::Continue(()))
                    })?;
                Ok(hits)
            })
            .collect::<Result<_>>()?;
        self.release_pages();
        Ok(per_class.into_iter().flatten().collect())
    }

    /// Visits every instruction of every method the filter admits for
    /// `query`, collecting each `Some`.
    pub fn scan_instructions<T: Send>(
        &self,
        query: &RefQuery,
        f: impl Fn(&InstructionSite<'_>) -> Option<T> + Sync,
    ) -> Result<Vec<T>> {
        let per_method: Vec<Vec<T>> = self.scan_methods_collect(query, |view| {
            let mut hits = Vec::new();
            let mut insn_idx = 0;
            view.for_each_instruction(|instruction| {
                if let Some(value) = f(&InstructionSite {
                    class_idx: view.class_idx,
                    method_pos: view.method_pos,
                    kind: view.kind,
                    insn_idx,
                    instruction,
                }) {
                    hits.push(value);
                }
                insn_idx += 1;
                true
            })?;
            Ok((!hits.is_empty()).then_some(hits))
        })?;
        Ok(per_method.into_iter().flatten().collect())
    }

    fn filter_for(&self, query: &RefQuery) -> Result<Option<&RefFilter>> {
        if query.is_empty() {
            return Ok(None);
        }
        self.ref_filter().map(Some)
    }

    fn scan_class<T>(
        &self,
        class_idx: usize,
        filter: Option<&RefFilter>,
        query: &RefQuery,
        visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        let class_type = self.classes.header(class_idx).class_type;
        if let Some(class) = self.classes.resident(class_idx) {
            return match &class.class_data {
                Some(data) => scan_resolved_class(class_idx, class_type, data, &mut |view| {
                    crate::references::check_index(
                        self,
                        crate::types::Pool::Method,
                        view.method.0,
                    )?;
                    visit(view)
                }),
                None => Ok(ControlFlow::Continue(())),
            };
        }
        match self.raw_class_data_offset(class_idx) {
            Some(offset) => {
                let filter = filter.map(|f| f.class(class_idx));
                self.scan_raw_class(class_idx, class_type, offset, filter, query, visit)
            }
            None => Ok(ControlFlow::Continue(())),
        }
    }

    fn scan_raw_class<T>(
        &self,
        class_idx: usize,
        class_type: TypeIdx,
        offset: u32,
        filter: Option<ClassFilter<'_>>,
        query: &RefQuery,
        visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        if filter.as_ref().is_some_and(|f| !f.admits_any(query)) {
            return Ok(ControlFlow::Continue(()));
        }
        let buf = self.raw_bytes(offset)?;
        let opts = self.parse_options;

        let mut cursor = ClassDataCursor::new(buf, offset as usize, opts)?;
        cursor.skip_fields()?;
        for (kind, count) in [
            (
                crate::types::class::MethodKind::Direct,
                cursor.counts.direct_methods,
            ),
            (
                crate::types::class::MethodKind::Virtual,
                cursor.counts.virtual_methods,
            ),
        ] {
            let slot_base = if kind == crate::types::class::MethodKind::Virtual {
                cursor.counts.direct_methods as usize
            } else {
                0
            };
            let mut method_pos = 0;
            let result = cursor.methods(count, |header| {
                crate::references::check_index(self, crate::types::Pool::Method, header.method.0)?;
                let position = method_pos;
                method_pos += 1;
                if filter
                    .as_ref()
                    .is_some_and(|f| !query.admits(f.method(slot_base + position)))
                {
                    return Ok(ControlFlow::Continue(()));
                }
                let view = MethodView {
                    method: header.method,
                    access_flags: header.access_flags,
                    class_idx,
                    class_type,
                    method_pos: position,
                    kind,
                    code: if header.code_off == 0 {
                        Code::None
                    } else {
                        Code::Raw {
                            buf,
                            code_off: header.code_off,
                        }
                    },
                };
                visit(&view)
            })?;
            if let ControlFlow::Break(value) = result {
                return Ok(ControlFlow::Break(value));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}

fn scan_resolved_class<T>(
    class_idx: usize,
    class_type: TypeIdx,
    data: &ClassData,
    visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
) -> Result<ControlFlow<T>> {
    let lists = [
        (
            crate::types::class::MethodKind::Direct,
            data.direct_methods.as_slice(),
        ),
        (
            crate::types::class::MethodKind::Virtual,
            data.virtual_methods.as_slice(),
        ),
    ];
    for (kind, methods) in lists {
        for (method_pos, method) in methods.iter().enumerate() {
            let code = match &method.code {
                Some(code) => Code::Resolved(&code.instructions),
                None => Code::None,
            };
            let view = MethodView {
                method: method.method,
                access_flags: method.access_flags,
                class_idx,
                class_type,
                method_pos,
                kind,
                code,
            };
            if let ControlFlow::Break(value) = visit(&view)? {
                return Ok(ControlFlow::Break(value));
            }
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn decode_one_method(
    source: &super::DexBytes,
    offset: u32,
    method_pos: usize,
    kind: crate::types::class::MethodKind,
    opts: ParseOptions,
) -> Result<Option<EncodedMethod>> {
    let mut cursor = ClassDataCursor::new(source.as_bytes(), offset as usize, opts)?;
    cursor.skip_fields()?;
    if kind == crate::types::class::MethodKind::Virtual {
        let _: ControlFlow<()> = cursor.methods::<()>(cursor.counts.direct_methods, |_| {
            Ok(ControlFlow::Continue(()))
        })?;
    }
    let count = if kind == crate::types::class::MethodKind::Virtual {
        cursor.counts.virtual_methods
    } else {
        cursor.counts.direct_methods
    };
    let mut position = 0;
    let result = cursor.methods(count, |header| {
        let selected = position == method_pos;
        position += 1;
        if !selected {
            return Ok(ControlFlow::Continue(()));
        }
        let code = (header.code_off != 0)
            .then(|| read_code_item(source, header.code_off, opts))
            .transpose()?;
        Ok(ControlFlow::Break(EncodedMethod {
            method: header.method,
            access_flags: header.access_flags,
            code,
        }))
    })?;
    Ok(result.break_value())
}
