// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::intern::ByteInterner;
use super::plan::{WriteClass, WritePlan};
use super::raw_code::{copy_code_item, copy_debug_info};
use super::remap::Remap;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::MethodIdx;
use crate::model::access_flags::AccessFlags;
use crate::model::class::{ClassData, EncodedField};
use crate::model::code::CodeItem;
use crate::model::map::{MapItem, TYPE_CODE_ITEM, TYPE_DEBUG_INFO_ITEM};
use crate::options::ParseOptions;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::read_code_item;

mod item;

use item::write_code_item;

pub(crate) struct MethodLayout {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code_off: u32,
}

pub(crate) struct ClassLayout {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<MethodLayout>,
    pub virtual_methods: Vec<MethodLayout>,
}

pub(crate) fn write_code_and_debug<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<Vec<Option<ClassLayout>>> {
    let code_start = w.pos();

    let mut emitter = CodeEmitter {
        debug: ByteInterner::new()?,
        code_debug_item: Vec::new(),
        scratch: Vec::new(),
        code_buf: Vec::new(),
    };
    let mut layouts: Vec<Option<ClassLayout>> = Vec::with_capacity(plan.classes.len());
    let remap = plan.remap();

    for class in &plan.classes {
        let layout = match class {
            WriteClass::Resident(class) => match &class.class_data {
                Some(data) => Some(emitter.write_resident(w, data, remap)?),
                None => None,
            },
            WriteClass::Raw(raw) if raw.class_data_off != 0 => {
                Some(emitter.write_deferred(w, plan, raw.class_data_off, remap)?)
            }
            WriteClass::Raw(_) => None,
        };
        layouts.push(layout);
    }

    let code_item_count = emitter.code_debug_item.len() as u32;
    if code_item_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_CODE_ITEM,
            size: code_item_count,
            offset: code_start,
        });
    }

    let debug_start = w.pos();
    if !emitter.debug.is_empty() {
        emitter.debug.write_to(&mut w.sink)?;
        w.map_entries.push(MapItem {
            type_code: TYPE_DEBUG_INFO_ITEM,
            size: emitter.debug.len() as u32,
            offset: debug_start,
        });
    }

    for &(code_off, item) in &emitter.code_debug_item {
        let value = item.map_or(0, |i| debug_start + emitter.debug.offset(i as usize));
        w.patch_u32(code_off as usize + 8, value);
    }

    Ok(layouts)
}

struct CodeEmitter {
    debug: ByteInterner,
    code_debug_item: Vec<(u32, Option<u32>)>,
    scratch: Vec<u8>,
    code_buf: Vec<u8>,
}

impl CodeEmitter {
    fn write_resident<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        data: &ClassData,
        remap: Option<&Remap>,
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
            let mut methods: Vec<_> = methods.iter().collect();
            let map = |method| remap.map_or(method, |remap| remap.remap_method(method));
            if remap.is_some() {
                methods.sort_by_key(|method| map(method.method));
            }
            for method in methods {
                let code_off = match &method.code {
                    Some(code) => {
                        if let Some(remap) = remap {
                            let mut code = CodeItem::clone(code);
                            if code.debug_info.as_ref().is_some_and(|metadata| {
                                !w.options.debug_info.keeps(metadata, w.original.as_ref())
                            }) {
                                code.debug_info = None;
                            }
                            remap.remap_code(&mut code)?;
                            super::remap::fixup_code(&mut code)?;
                            self.write_code(w, &code)?
                        } else {
                            self.write_code(w, code)?
                        }
                    }
                    None => 0,
                };
                entries.push(MethodLayout {
                    method: map(method.method),
                    access_flags: method.access_flags,
                    code_off,
                });
            }
        }
        if let Some(remap) = remap {
            for fields in [&mut layout.static_fields, &mut layout.instance_fields] {
                for field in &mut *fields {
                    field.field = remap.remap_field(field.field);
                }
                fields.sort_by_key(|field| field.field);
            }
        }
        Ok(layout)
    }

    fn write_deferred<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        offset: u32,
        remap: Option<&Remap>,
    ) -> Result<ClassLayout> {
        let buf = plan.raw_bytes();
        let opts = plan.dex.parse_options;
        let mut skeleton = read_class_skeleton_at(buf, offset as usize, opts)?;
        crate::references::validate_skeleton(plan.dex, &skeleton)?;

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
                let code_off = if header.code_off == 0 {
                    0
                } else {
                    self.write_file_code(
                        w,
                        plan.dex.raw.as_ref().expect("raw code retains source"),
                        header.code_off,
                        remap,
                        opts,
                    )?
                };
                entries.push(MethodLayout {
                    method: header.method,
                    access_flags: header.access_flags,
                    code_off,
                });
            }
        }
        Ok(layout)
    }

    fn write_file_code<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        source: &crate::DexBytes,
        code_off: u32,
        remap: Option<&Remap>,
        opts: ParseOptions,
    ) -> Result<u32> {
        let buf = source.as_bytes();
        self.code_buf.clear();
        if copy_code_item(buf, code_off, remap, opts, &mut self.code_buf)? {
            w.align(4);
            let off = w.pos();
            w.write(&self.code_buf);
            let debug_off = crate::read::u32_at(buf, code_off as usize + 8);
            let item = if debug_off != 0
                && w.options.debug_info == crate::options::MetadataPolicy::Preserve
            {
                self.scratch.clear();
                copy_debug_info(buf, debug_off, remap, opts, &mut self.scratch)?;
                Some(self.debug.intern(&self.scratch)? as u32)
            } else {
                None
            };
            self.code_debug_item.push((off, item));
            return Ok(off);
        }
        let mut code = read_code_item(source, code_off, opts)?;
        if code
            .debug_info
            .as_ref()
            .is_some_and(|metadata| !w.options.debug_info.keeps(metadata, w.original.as_ref()))
        {
            code.debug_info = None;
        }
        if let Some(remap) = remap {
            remap.remap_code(&mut code)?;
            super::remap::fixup_code(&mut code)?;
        }
        self.write_code(w, &code)
    }

    fn write_code<S: DexSink>(&mut self, w: &mut DexWriter<S>, code: &CodeItem) -> Result<u32> {
        w.align(4);
        let off = w.pos();
        write_code_item(w, code)?;
        let item = code
            .debug_info
            .as_ref()
            .filter(|metadata| w.options.debug_info.keeps(metadata, w.original.as_ref()))
            .map(|debug| {
                self.scratch.clear();
                super::debug::write_debug_info(&mut self.scratch, debug.read()?.as_ref());
                Ok::<_, crate::DexError>(self.debug.intern(&self.scratch)? as u32)
            })
            .transpose()?;
        self.code_debug_item.push((off, item));
        Ok(off)
    }
}
