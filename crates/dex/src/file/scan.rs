// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Method scanning that reads straight from the raw DEX bytes.
//!
//! Searches never build the decoded IR. A class whose `class_data` is
//! already materialized (a patch touched it, created it, or it came from an
//! eagerly-parsed DEX) is scanned through that IR, which may be modified.
//! Every other class is streamed from the original buffer and its methods are
//! walked one instruction at a time, reading only the opcode and the operand
//! the search asks about.

use std::ops::ControlFlow;

use rayon::prelude::*;

use super::pattern::{find_pattern_span, InstructionPattern};
use super::ref_filter::RefFilter;
use super::{DexFile, RefKey, RefQuery};
use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::Result;
use crate::read::class::{read_class_skeleton_at, ClassSkeleton, MethodHeader};
use crate::read::code::{count_instructions, read_code_item, walk_instructions, RawInstruction};
use crate::read::header::{u16_at, u32_at};
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
    pub is_virtual: bool,
    code: Code<'a>,
}

enum Code<'a> {
    /// Abstract or native: no code item.
    None,
    /// From materialized IR (possibly mutated by an earlier patch).
    Resolved(&'a [Instruction]),
    /// Still in the raw buffer, walked in place.
    Raw { buf: &'a [u8], code_off: u32 },
}

impl<'a> MethodView<'a> {
    pub fn has_code(&self) -> bool {
        !matches!(self.code, Code::None)
    }

    /// Visits each instruction in order, stopping when `visit` returns `false`.
    pub fn for_each_instruction(&self, mut visit: impl FnMut(InstructionRef<'_>) -> bool) -> Result<()> {
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
                let insns_size = u32_at(buf, base + 12)? as usize;
                walk_instructions(buf, base + 16, insns_size, |insn| {
                    visit(InstructionRef::Raw { buf, insn: *insn })
                })
            }
        }
    }

    /// Whether any instruction satisfies `pred`.
    pub fn any_instruction(&self, mut pred: impl FnMut(InstructionRef<'_>) -> bool) -> Result<bool> {
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
            is_virtual: self.is_virtual,
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
    pub is_virtual: bool,
}

/// A located instruction match within a method.
#[derive(Debug, Clone)]
pub struct InstructionHit {
    pub class_idx: usize,
    pub method_pos: usize,
    pub is_virtual: bool,
    pub insn_idx: usize,
}

/// One instruction visited during an instruction scan.
pub struct InstructionSite<'a> {
    pub class_idx: usize,
    pub method_pos: usize,
    pub is_virtual: bool,
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
                let _: ControlFlow<()> = self.scan_class(class_idx, filter, query, &mut |view| {
                    if let Some(value) = f(view)? {
                        hits.push(value);
                    }
                    Ok(ControlFlow::Continue(()))
                })?;
                Ok(hits)
            })
            .collect::<Result<_>>()?;
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
                    is_virtual: view.is_virtual,
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

    /// Dispatches one class to resolved or raw scanning.
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
                Some(data) => scan_resolved_class(class_idx, class_type, data, visit),
                None => Ok(ControlFlow::Continue(())),
            };
        }
        match self.raw_class_data_offset(class_idx) {
            Some(offset) => {
                let masks = filter.map(|f| f.class(class_idx));
                self.scan_raw_class(class_idx, class_type, offset, masks, query, visit)
            }
            None => Ok(ControlFlow::Continue(())),
        }
    }

    fn scan_raw_class<T>(
        &self,
        class_idx: usize,
        class_type: TypeIdx,
        offset: u32,
        masks: Option<&[u64]>,
        query: &RefQuery,
        visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        if masks.is_some_and(|masks| !masks.iter().any(|&m| query.admits(m))) {
            return Ok(ControlFlow::Continue(()));
        }
        let buf = self.raw_bytes(offset)?;
        let opts = &self.parse_options;

        let mut pos = offset as usize;
        let (static_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (instance_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (direct_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (virtual_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;

        pos = skip_encoded_fields(buf, pos, static_fields_size, opts)?;
        pos = skip_encoded_fields(buf, pos, instance_fields_size, opts)?;

        for is_virtual in [false, true] {
            let count = if is_virtual {
                virtual_methods_size
            } else {
                direct_methods_size
            };
            // The method_idx_diff of the first entry in each list is absolute.
            let mut method_idx = 0u32;
            let slot_base = if is_virtual { direct_methods_size as usize } else { 0 };
            for method_pos in 0..count as usize {
                let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;
                method_idx = method_idx.wrapping_add(diff);
                let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;
                let (code_off, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;

                if masks.is_some_and(|masks| !query.admits(masks[slot_base + method_pos])) {
                    continue;
                }
                let code = if code_off != 0 {
                    Code::Raw { buf, code_off }
                } else {
                    Code::None
                };
                let view = MethodView {
                    method: MethodIdx(method_idx),
                    access_flags: AccessFlags::from_bits_retain(access),
                    class_idx,
                    class_type,
                    method_pos,
                    is_virtual,
                    code,
                };
                if let ControlFlow::Break(value) = visit(&view)? {
                    return Ok(ControlFlow::Break(value));
                }
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
        (false, data.direct_methods.as_slice()),
        (true, data.virtual_methods.as_slice()),
    ];
    for (is_virtual, methods) in lists {
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
                is_virtual,
                code,
            };
            if let ControlFlow::Break(value) = visit(&view)? {
                return Ok(ControlFlow::Break(value));
            }
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn skip_encoded_fields(
    buf: &[u8],
    mut pos: usize,
    count: u32,
    opts: &ParseOptions,
) -> Result<usize> {
    for _ in 0..count {
        let (_, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (_, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
    }
    Ok(pos)
}

fn read_raw_fields(
    buf: &[u8],
    mut pos: usize,
    count: u32,
    opts: &ParseOptions,
) -> Result<(Vec<EncodedField>, usize)> {
    let mut fields = Vec::with_capacity(count as usize);
    let mut field_idx = 0u32;
    for _ in 0..count {
        let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        field_idx = field_idx.wrapping_add(diff);
        let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        fields.push(EncodedField {
            field: FieldIdx(field_idx),
            access_flags: AccessFlags::from_bits_retain(access),
        });
    }
    Ok((fields, pos))
}

/// Walks a raw `class_data` to a single method position and fully decodes it
/// (including code, tries, and debug info per the parse options). Only the
/// target method's code is decoded; the rest are skipped by their headers.
fn decode_one_method(
    buf: &[u8],
    offset: u32,
    method_pos: usize,
    is_virtual: bool,
    opts: &ParseOptions,
) -> Result<Option<EncodedMethod>> {
    let skeleton = read_class_skeleton_at(buf, offset as usize, opts)?;
    let Some(header) = skeleton.method(method_pos, is_virtual) else {
        return Ok(None);
    };
    let code = if header.code_off != 0 {
        Some(read_code_item(buf, header.code_off, opts)?)
    } else {
        None
    };
    Ok(Some(EncodedMethod {
        method: header.method,
        access_flags: header.access_flags,
        code,
    }))
}

impl ClassSkeleton {
    pub fn method(&self, method_pos: usize, is_virtual: bool) -> Option<&MethodHeader> {
        if is_virtual {
            self.virtual_methods.get(method_pos)
        } else {
            self.direct_methods.get(method_pos)
        }
    }
}

/// A method's identity and frame shape, read without decoding its code.
#[derive(Debug, Clone, Copy)]
pub struct MethodSummary {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub has_code: bool,
    pub instruction_count: u32,
}

impl DexFile {
    /// The member lists of a still-deferred class, read from the raw buffer
    /// without touching any code. `None` for materialized classes and classes
    /// without class data.
    pub fn class_skeleton(&self, class_idx: usize) -> Result<Option<ClassSkeleton>> {
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(None);
        };
        let buf = self.raw_bytes(offset)?;
        Ok(Some(read_class_skeleton_at(buf, offset as usize, &self.parse_options)?))
    }

    /// Summarizes a deferred method from its skeleton entry: the 16-byte code
    /// item header plus an opcode-length walk for the instruction count.
    pub fn summarize_method(&self, header: &MethodHeader) -> Result<MethodSummary> {
        let mut summary = MethodSummary {
            method: header.method,
            access_flags: header.access_flags,
            registers_size: 0,
            ins_size: 0,
            outs_size: 0,
            has_code: header.code_off != 0,
            instruction_count: 0,
        };
        if header.code_off != 0 {
            let buf = self.raw_bytes(header.code_off)?;
            let base = header.code_off as usize;
            summary.registers_size = u16_at(buf, base)?;
            summary.ins_size = u16_at(buf, base + 2)?;
            summary.outs_size = u16_at(buf, base + 4)?;
            let insns_size = u32_at(buf, base + 12)? as usize;
            summary.instruction_count = count_instructions(buf, base + 16, insns_size)?;
        }
        Ok(summary)
    }

    /// Summarizes one method: from IR when its class is materialized, else via
    /// [`Self::class_skeleton`] and [`Self::summarize_method`].
    pub fn method_summary(
        &self,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> Result<Option<MethodSummary>> {
        if let Some(data) = self.resident_class_data(class_idx) {
            let list = if is_virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return Ok(list.get(method_pos).map(summarize_resident));
        }
        let Some(skeleton) = self.class_skeleton(class_idx)? else {
            return Ok(None);
        };
        skeleton
            .method(method_pos, is_virtual)
            .map(|header| self.summarize_method(header))
            .transpose()
    }
}

pub fn summarize_resident(m: &EncodedMethod) -> MethodSummary {
    let code = m.code.as_ref();
    MethodSummary {
        method: m.method,
        access_flags: m.access_flags,
        registers_size: code.map_or(0, |c| c.registers_size),
        ins_size: code.map_or(0, |c| c.ins_size),
        outs_size: code.map_or(0, |c| c.outs_size),
        has_code: code.is_some(),
        instruction_count: code.map_or(0, |c| c.instructions.len() as u32),
    }
}

/// Member counts of a class, needed by inspection FFIs that never touch code.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemberCounts {
    pub direct_methods: u32,
    pub virtual_methods: u32,
    pub static_fields: u32,
    pub instance_fields: u32,
}

impl DexFile {
    /// Class member counts without materializing the class. For a deferred
    /// class this reads only the four `class_data` header LEBs — no field,
    /// method, or code decoding.
    pub fn class_member_counts(&self, class_idx: usize) -> Result<Option<MemberCounts>> {
        if class_idx >= self.classes.len() {
            return Ok(None);
        }
        if let Some(data) = self.resident_class_data(class_idx) {
            return Ok(Some(MemberCounts {
                direct_methods: data.direct_methods.len() as u32,
                virtual_methods: data.virtual_methods.len() as u32,
                static_fields: data.static_fields.len() as u32,
                instance_fields: data.instance_fields.len() as u32,
            }));
        }
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(Some(MemberCounts::default()));
        };
        let buf = self.raw_bytes(offset)?;
        let opts = &self.parse_options;
        let mut pos = offset as usize;
        let (static_fields, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (instance_fields, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (direct_methods, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (virtual_methods, _) = read_uleb128_with_opts(buf, pos, opts)?;
        Ok(Some(MemberCounts {
            direct_methods,
            virtual_methods,
            static_fields,
            instance_fields,
        }))
    }

    /// Class fields `(static, instance)` without materializing the class. For a
    /// deferred class this decodes only the encoded-field entries, never methods
    /// or code.
    pub fn decode_class_fields(
        &self,
        class_idx: usize,
    ) -> Result<Option<(Vec<EncodedField>, Vec<EncodedField>)>> {
        if class_idx >= self.classes.len() {
            return Ok(None);
        }
        if let Some(data) = self.resident_class_data(class_idx) {
            return Ok(Some((
                data.static_fields.clone(),
                data.instance_fields.clone(),
            )));
        }
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(Some((Vec::new(), Vec::new())));
        };
        let buf = self.raw_bytes(offset)?;
        let opts = &self.parse_options;
        let mut pos = offset as usize;
        let (static_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (instance_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        // Skip the two method-count LEBs; methods and code are not decoded.
        let (_, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (_, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        let (static_fields, new_pos) = read_raw_fields(buf, pos, static_fields_size, opts)?;
        pos = new_pos;
        let (instance_fields, _) = read_raw_fields(buf, pos, instance_fields_size, opts)?;
        Ok(Some((static_fields, instance_fields)))
    }

    /// The `class_data_item` offset of a class still in the file; `None` for
    /// resident classes and classes without class data.
    pub(crate) fn raw_class_data_offset(&self, class_idx: usize) -> Option<u32> {
        self.classes
            .raw_def(class_idx)
            .map(|def| def.class_data_off)
            .filter(|&off| off != 0)
    }

    /// The class data of a resident class, when it is resident and has any.
    /// `None` also for classes still in the file: callers fall back to the
    /// raw readers, and a resident class without class data has no members.
    fn resident_class_data(&self, class_idx: usize) -> Option<&ClassData> {
        self.classes.resident(class_idx)?.class_data.as_deref()
    }

    pub(crate) fn raw_bytes(&self, offset: u32) -> Result<&[u8]> {
        Ok(self
            .raw
            .as_ref()
            .ok_or_else(|| crate::error::invalid_offset("class data", offset, 0))?
            .as_bytes())
    }

    /// Decodes one method at a position without persisting its class.
    ///
    /// If the class is already materialized, clones from its IR; otherwise
    /// decodes just that one method from the raw buffer, leaving the class
    /// deferred. Read-only inspection uses this so reading a method never
    /// permanently materializes the rest of its class.
    pub fn decode_method_at(
        &self,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> Result<Option<EncodedMethod>> {
        if let Some(data) = self.resident_class_data(class_idx) {
            let list = if is_virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return Ok(list.get(method_pos).cloned());
        }

        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(None);
        };
        let buf = self.raw_bytes(offset)?;
        decode_one_method(buf, offset, method_pos, is_virtual, &self.parse_options)
    }
}

/// Method-level search entry points built on the scan core.
impl DexFile {
    /// Finds the first method with the given name.
    pub fn find_method_by_name(&self, name: &str) -> Result<Option<MethodHit>> {
        let Some(name) = self.find_string_idx(name) else {
            return Ok(None);
        };
        self.scan_methods_find(&RefQuery::default(), |view| {
            Ok((self.method_id(view.method).name == name).then(|| view.hit()))
        })
    }

    /// Finds every method whose body loads all of the given string constants.
    ///
    /// The string set is resolved to indices once per DEX; a DEX missing any of
    /// them is rejected before a single method is scanned.
    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Result<Vec<MethodHit>> {
        let targets: Vec<StringIdx> = strings
            .iter()
            .filter_map(|s| self.find_string_idx(s))
            .collect();
        if targets.len() != strings.len() {
            return Ok(Vec::new());
        }

        let query = RefQuery::all_of(targets.iter().map(|&s| RefKey::string(s)));
        self.scan_methods_collect(&query, |view| {
            if !view.has_code() {
                return Ok(None);
            }
            for target in &targets {
                if !view.any_instruction(|insn| insn.string_ref() == Some(*target))? {
                    return Ok(None);
                }
            }
            Ok(Some(view.hit()))
        })
    }

    /// Finds every method whose instructions match the opcode pattern.
    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodHit>> {
        self.scan_methods_collect(&RefQuery::default(), |view| {
            if !view.has_code() {
                return Ok(None);
            }
            let mut seq = Vec::new();
            view.opcodes(&mut seq)?;
            Ok(find_pattern_span(&seq, opcodes).is_some().then(|| view.hit()))
        })
    }
}
