// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction_writer::encode_instructions;
use super::intern::ByteInterner;
use super::plan::{WriteClass, WritePlan};
use super::raw_code::{copy_code_item, copy_debug_info};
use super::sink::DexSink;
use super::sort::Remap;
use super::DexWriter;
use crate::encoding::leb128::{write_sleb128, write_uleb128};
use crate::error::Result;
use crate::types::access_flags::AccessFlags;
use crate::types::header::ParseOptions;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::read_code_item;
use crate::types::class::{ClassData, EncodedField};
use crate::types::code::CodeItem;
use crate::types::map::{MapItem, TYPE_CODE_ITEM, TYPE_DEBUG_INFO_ITEM};
use crate::types::MethodIdx;

/// A method's class-data entry with its code stripped: enough to emit the
/// `class_data_item` after the code items are laid out, without keeping any
/// decoded instructions resident.
pub(crate) struct MethodLayout {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub has_code: bool,
}

/// A class's `class_data_item` shape, retained while its (heavy) code is
/// dropped. Cheap: a few integers per member.
pub(crate) struct ClassLayout {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<MethodLayout>,
    pub virtual_methods: Vec<MethodLayout>,
}

/// Writes all code items and their deduplicated debug info in a single pass
/// and returns the per-class layout the class-data section needs afterwards.
///
/// Resident classes (a patch touched them) were already remapped and widened
/// by the write plan and are written from their IR. A file class is read
/// from the original buffer as a skeleton, its member indices remapped and
/// re-sorted exactly as the resident path does, and each method's code item
/// is copied with its pool indices rewritten in place; only a method whose
/// `const-string` outgrows 16 bits goes through decode, widen and encode.
///
/// Layout matches the eager writer byte-for-byte: every `code_item` first (in
/// class-then-direct-then-virtual order), then the contiguous debug-info
/// section, then the code items' `debug_info_off` fields backpatched.
pub(crate) fn write_code_and_debug<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<Vec<Option<ClassLayout>>> {
    let code_start = w.pos();
    w.code_item_offsets.clear();

    let mut emitter = CodeEmitter {
        debug: ByteInterner::default(),
        code_debug_item: Vec::new(),
        scratch: Vec::new(),
        code_buf: Vec::new(),
    };
    let mut layouts: Vec<Option<ClassLayout>> = Vec::with_capacity(plan.classes.len());
    let remap = plan.remap();

    for class in &plan.classes {
        let layout = match class {
            WriteClass::Resident(class) => match &class.class_data {
                Some(data) => Some(emitter.write_resident(w, data)?),
                None => None,
            },
            WriteClass::Raw(raw) if raw.class_data_off != 0 => {
                Some(emitter.write_deferred(w, plan, raw.class_data_off, remap.as_ref())?)
            }
            WriteClass::Raw(_) => None,
        };
        layouts.push(layout);
    }

    let code_item_count = w.code_item_offsets.len() as u32;
    if code_item_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_CODE_ITEM,
            size: code_item_count,
            offset: code_start,
        });
    }

    let debug_start = w.pos();
    if !emitter.debug.is_empty() {
        w.write(emitter.debug.data());
        w.map_entries.push(MapItem {
            type_code: TYPE_DEBUG_INFO_ITEM,
            size: emitter.debug.len() as u32,
            offset: debug_start,
        });
    }

    for (ci_idx, item) in emitter.code_debug_item.iter().enumerate() {
        let ci_off = w.code_item_offsets[ci_idx] as usize;
        let value = item.map_or(0, |i| debug_start + emitter.debug.offset(i as usize));
        w.patch_u32(ci_off + 8, value);
    }

    Ok(layouts)
}

struct CodeEmitter {
    debug: ByteInterner,
    /// Per code item, the index of its debug item in `debug`.
    code_debug_item: Vec<Option<u32>>,
    scratch: Vec<u8>,
    code_buf: Vec<u8>,
}

impl CodeEmitter {
    fn write_resident<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        data: &ClassData,
    ) -> Result<ClassLayout> {
        let mut layout = ClassLayout {
            static_fields: data.static_fields.clone(),
            instance_fields: data.instance_fields.clone(),
            direct_methods: Vec::with_capacity(data.direct_methods.len()),
            virtual_methods: Vec::with_capacity(data.virtual_methods.len()),
        };
        for (methods, entries) in [
            (&data.direct_methods, &mut layout.direct_methods),
            (&data.virtual_methods, &mut layout.virtual_methods),
        ] {
            for method in methods {
                if let Some(code) = &method.code {
                    self.write_code(w, code)?;
                }
                entries.push(MethodLayout {
                    method: method.method,
                    access_flags: method.access_flags,
                    has_code: method.code.is_some(),
                });
            }
        }
        Ok(layout)
    }

    fn write_deferred<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        offset: u32,
        remap: Option<&Remap<'_>>,
    ) -> Result<ClassLayout> {
        let buf = plan.raw_bytes();
        let opts = &plan.dex.parse_options;
        let mut skeleton = read_class_skeleton_at(buf, offset as usize, opts)?;

        if let Some(remap) = remap {
            for field in skeleton
                .static_fields
                .iter_mut()
                .chain(skeleton.instance_fields.iter_mut())
            {
                field.field = remap.remap_field(field.field);
            }
            skeleton.static_fields.sort_by_key(|f| f.field.0);
            skeleton.instance_fields.sort_by_key(|f| f.field.0);
            for header in skeleton
                .direct_methods
                .iter_mut()
                .chain(skeleton.virtual_methods.iter_mut())
            {
                header.method = remap.remap_method(header.method);
            }
            skeleton.direct_methods.sort_by_key(|m| m.method.0);
            skeleton.virtual_methods.sort_by_key(|m| m.method.0);
        }

        let mut layout = ClassLayout {
            static_fields: skeleton.static_fields,
            instance_fields: skeleton.instance_fields,
            direct_methods: Vec::with_capacity(skeleton.direct_methods.len()),
            virtual_methods: Vec::with_capacity(skeleton.virtual_methods.len()),
        };
        for (headers, entries) in [
            (&skeleton.direct_methods, &mut layout.direct_methods),
            (&skeleton.virtual_methods, &mut layout.virtual_methods),
        ] {
            for header in headers {
                if header.code_off != 0 {
                    self.write_file_code(w, buf, header.code_off, remap, opts)?;
                }
                entries.push(MethodLayout {
                    method: header.method,
                    access_flags: header.access_flags,
                    has_code: header.code_off != 0,
                });
            }
        }
        Ok(layout)
    }

    /// Writes a file class's method from its bytes, falling back to the
    /// decoding path when an operand has to be widened.
    fn write_file_code<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        buf: &[u8],
        code_off: u32,
        remap: Option<&Remap<'_>>,
        opts: &ParseOptions,
    ) -> Result<()> {
        self.code_buf.clear();
        if copy_code_item(buf, code_off, remap, opts, &mut self.code_buf)? {
            w.align(4);
            w.code_item_offsets.push(w.pos());
            w.write(&self.code_buf);
            let debug_off = u32::from_le_bytes(buf[code_off as usize + 8..code_off as usize + 12].try_into().unwrap());
            let item = if debug_off != 0 && opts.include_debug_info {
                self.scratch.clear();
                copy_debug_info(buf, debug_off, remap, opts, &mut self.scratch)?;
                Some(self.debug.intern(&self.scratch) as u32)
            } else {
                None
            };
            self.code_debug_item.push(item);
            return Ok(());
        }
        let mut code = read_code_item(buf, code_off, opts)?;
        if let Some(remap) = remap {
            remap.remap_code(&mut code);
            super::sort::fixup_code(&mut code)?;
        }
        self.write_code(w, &code)
    }

    fn write_code<S: DexSink>(&mut self, w: &mut DexWriter<S>, code: &CodeItem) -> Result<()> {
        w.align(4);
        let off = w.pos();
        w.code_item_offsets.push(off);
        write_code_item(w, code)?;
        let item = code.debug_info.as_ref().map(|debug| {
            self.scratch.clear();
            super::debug::write_debug_info(&mut self.scratch, debug);
            self.debug.intern(&self.scratch) as u32
        });
        self.code_debug_item.push(item);
        Ok(())
    }
}

/// Writes one `code_item`, including the shared encoded catch-handler stream.
pub(crate) fn write_code_item<S: DexSink>(w: &mut DexWriter<S>, code: &CodeItem) -> Result<()> {
    w.write_u16(code.registers_size);
    w.write_u16(code.ins_size);
    w.write_u16(code.compute_outs_size());
    w.write_u16(code.tries.len() as u16);
    w.write_u32(0);
    let insns = encode_instructions(&code.instructions)?;
    w.write_u32(insns.len() as u32);
    for unit in &insns {
        w.write_u16(*unit);
    }

    if !code.tries.is_empty() {
        if !insns.len().is_multiple_of(2) {
            w.write_u16(0);
        }

        let mut handler_buf: Vec<u8> = Vec::new();
        write_uleb128(&mut handler_buf, code.catch_handlers.len() as u32);
        let mut handler_byte_offsets: Vec<u16> = Vec::new();
        for handler in &code.catch_handlers {
            handler_byte_offsets.push(handler_buf.len() as u16);
            let size = if handler.catch_all_addr.is_some() {
                -(handler.typed_catches.len() as i32)
            } else {
                handler.typed_catches.len() as i32
            };
            write_sleb128(&mut handler_buf, size);
            for tc in &handler.typed_catches {
                write_uleb128(&mut handler_buf, tc.exception_type.0);
                write_uleb128(&mut handler_buf, tc.addr);
            }
            if let Some(addr) = handler.catch_all_addr {
                write_uleb128(&mut handler_buf, addr);
            }
        }

        for t in &code.tries {
            w.write_u32(t.start_addr);
            w.write_u16(t.insn_count);
            w.write_u16(handler_byte_offsets[t.handler_idx]);
        }

        w.write(&handler_buf);
    }
    Ok(())
}
