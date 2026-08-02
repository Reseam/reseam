// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Method scanning that reads straight from the raw DEX bytes.
//!
//! Searches do not need the full decoded IR. A class whose `class_data` is
//! already materialized — because a patch touched it, created it, or it came
//! from an eagerly-parsed DEX — is scanned through that IR (which may be
//! modified). Every other class is streamed from the original buffer, decoding
//! one method's instructions at a time into a reusable scratch buffer, and only
//! when a predicate actually asks for them.
//!
//! The result is that a search holds one method's instructions at a time rather
//! than every method in the APK, and a name- or flag-only search never decodes
//! a single instruction.

use std::ops::ControlFlow;

use rayon::prelude::*;

use super::pattern::matches_pattern;
use super::{DexFile, InstructionPattern};
use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::Result;
use crate::read::code::{read_code_instructions_into, read_code_item};
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, EncodedField, EncodedMethod};
use crate::types::header::ParseOptions;
use crate::types::instruction::Instruction;
use crate::types::{FieldIdx, MethodIdx, StringIdx, TypeIdx};

/// A method encountered during a scan. Instructions decode on demand, so cheap
/// checks (name, flags, prototype) never pay for instruction decoding.
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
    /// Streamed from the raw buffer; decoded into `scratch` on first request.
    Raw {
        buf: &'a [u8],
        code_off: u32,
        scratch: &'a mut Vec<Instruction>,
        decoded: bool,
    },
}

impl<'a> MethodView<'a> {
    pub fn has_code(&self) -> bool {
        !matches!(self.code, Code::None)
    }

    /// Returns the method's instructions, decoding raw bytes on first call.
    /// Empty for a codeless method.
    pub fn instructions(&mut self) -> Result<&[Instruction]> {
        match &mut self.code {
            Code::None => Ok(&[]),
            Code::Resolved(instructions) => Ok(instructions),
            Code::Raw {
                buf,
                code_off,
                scratch,
                decoded,
            } => {
                if !*decoded {
                    read_code_instructions_into(buf, *code_off, scratch)?;
                    *decoded = true;
                }
                Ok(scratch)
            }
        }
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
    pub instruction: &'a Instruction,
}

impl DexFile {
    /// Scans methods across all classes, returning the first `Some`. Sequential
    /// with early exit, reusing one instruction buffer for the whole walk.
    pub fn scan_methods_find<T>(
        &self,
        mut f: impl FnMut(&mut MethodView<'_>) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let mut scratch = Vec::new();
        for class_idx in 0..self.classes.len() {
            let flow = self.scan_class(class_idx, &mut scratch, &mut |view| {
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
    /// in class-then-method order. Each rayon worker keeps its own buffer.
    pub fn scan_methods_collect<T: Send>(
        &self,
        f: impl Fn(&mut MethodView<'_>) -> Result<Option<T>> + Sync,
    ) -> Result<Vec<T>> {
        let per_class: Vec<Vec<T>> = (0..self.classes.len())
            .into_par_iter()
            .map_init(Vec::new, |scratch, class_idx| {
                let mut hits = Vec::new();
                let _: ControlFlow<()> = self.scan_class(class_idx, scratch, &mut |view| {
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

    /// Visits every instruction of every method, collecting each `Some`.
    pub fn scan_instructions<T: Send>(
        &self,
        f: impl Fn(&InstructionSite<'_>) -> Option<T> + Sync,
    ) -> Result<Vec<T>> {
        let per_method: Vec<Vec<T>> = self.scan_methods_collect(|view| {
            let (class_idx, method_pos, is_virtual) =
                (view.class_idx, view.method_pos, view.is_virtual);
            let mut hits = Vec::new();
            for (insn_idx, instruction) in view.instructions()?.iter().enumerate() {
                if let Some(value) = f(&InstructionSite {
                    class_idx,
                    method_pos,
                    is_virtual,
                    insn_idx,
                    instruction,
                }) {
                    hits.push(value);
                }
            }
            Ok((!hits.is_empty()).then_some(hits))
        })?;
        Ok(per_method.into_iter().flatten().collect())
    }

    /// Dispatches one class to resolved or raw scanning.
    fn scan_class<T>(
        &self,
        class_idx: usize,
        scratch: &mut Vec<Instruction>,
        visit: &mut impl FnMut(&mut MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        let class = &self.classes[class_idx];
        if let Some(data) = &class.class_data {
            return scan_resolved_class(class_idx, class.class_type, data, visit);
        }

        match self
            .lazy_class_data_offsets
            .as_ref()
            .and_then(|offsets| offsets.get(class_idx).copied())
        {
            Some(offset) if offset != 0 => {
                self.scan_raw_class(class_idx, class.class_type, offset, scratch, visit)
            }
            _ => Ok(ControlFlow::Continue(())),
        }
    }

    fn scan_raw_class<T>(
        &self,
        class_idx: usize,
        class_type: TypeIdx,
        offset: u32,
        scratch: &mut Vec<Instruction>,
        visit: &mut impl FnMut(&mut MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        let buf = self
            .raw
            .as_ref()
            .ok_or_else(|| crate::error::invalid_offset("scan class data", offset, 0))?
            .as_bytes();
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
            for method_pos in 0..count as usize {
                let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;
                method_idx = method_idx.wrapping_add(diff);
                let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;
                let (code_off, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;

                scratch.clear();
                let code = if code_off != 0 {
                    Code::Raw {
                        buf,
                        code_off,
                        scratch,
                        decoded: false,
                    }
                } else {
                    Code::None
                };
                let mut view = MethodView {
                    method: MethodIdx(method_idx),
                    access_flags: AccessFlags::from_bits_retain(access),
                    class_idx,
                    class_type,
                    method_pos,
                    is_virtual,
                    code,
                };
                if let ControlFlow::Break(value) = visit(&mut view)? {
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
    visit: &mut impl FnMut(&mut MethodView<'_>) -> Result<ControlFlow<T>>,
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
            let mut view = MethodView {
                method: method.method,
                access_flags: method.access_flags,
                class_idx,
                class_type,
                method_pos,
                is_virtual,
                code,
            };
            if let ControlFlow::Break(value) = visit(&mut view)? {
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

    for list_is_virtual in [false, true] {
        let count = if list_is_virtual {
            virtual_methods_size
        } else {
            direct_methods_size
        };
        let mut method_idx = 0u32;
        for pos_in_list in 0..count as usize {
            let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;
            method_idx = method_idx.wrapping_add(diff);
            let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;
            let (code_off, n) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += n;

            if list_is_virtual == is_virtual && pos_in_list == method_pos {
                let code = if code_off != 0 {
                    Some(read_code_item(buf, code_off, opts)?)
                } else {
                    None
                };
                return Ok(Some(EncodedMethod {
                    method: MethodIdx(method_idx),
                    access_flags: AccessFlags::from_bits_retain(access),
                    code,
                }));
            }
        }
    }
    Ok(None)
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
        let Some(class) = self.classes.get(class_idx) else {
            return Ok(None);
        };
        if let Some(data) = &class.class_data {
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
        let Some(class) = self.classes.get(class_idx) else {
            return Ok(None);
        };
        if let Some(data) = &class.class_data {
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

    pub(crate) fn raw_class_data_offset(&self, class_idx: usize) -> Option<u32> {
        self.lazy_class_data_offsets
            .as_ref()
            .and_then(|offsets| offsets.get(class_idx).copied())
            .filter(|&off| off != 0)
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
        let Some(class) = self.classes.get(class_idx) else {
            return Ok(None);
        };
        if let Some(data) = &class.class_data {
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
        self.scan_methods_find(|view| {
            let method_id = &self.methods[view.method.0 as usize];
            Ok((self.string(method_id.name) == name).then(|| view.hit()))
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

        self.scan_methods_collect(|view| {
            if !view.has_code() {
                return Ok(None);
            }
            let instructions = view.instructions()?;
            let all_present = targets.iter().all(|target| {
                instructions.iter().any(|insn| match insn {
                    Instruction::ConstString { string, .. }
                    | Instruction::ConstStringJumbo { string, .. } => string == target,
                    _ => false,
                })
            });
            Ok(all_present.then(|| view.hit()))
        })
    }

    /// Finds every method whose instructions match the opcode pattern.
    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodHit>> {
        self.scan_methods_collect(|view| {
            if !view.has_code() {
                return Ok(None);
            }
            let instructions = view.instructions()?;
            Ok(matches_pattern(instructions, opcodes).then(|| view.hit()))
        })
    }
}
