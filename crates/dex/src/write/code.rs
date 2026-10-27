// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::intern::ByteInterner;
use super::plan::{WriteClass, WritePlan};
use super::raw_code::{RawCopy, copy_code_item, copy_debug_info};
use super::remap::Remap;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::MethodIdx;
use crate::model::access_flags::AccessFlags;
use crate::model::class::{ClassData, EncodedField, EncodedMethod};
use crate::model::code::CodeItem;
use crate::model::map::{TYPE_CODE_ITEM, TYPE_DEBUG_INFO_ITEM};
use crate::options::MetadataPolicy;
use crate::read::class::{ClassSkeleton, MethodHeader, read_class_skeleton_at};
use crate::read::code::read_code_item;

mod item;

use item::write_code_item;

pub(crate) fn write_code_and_debug<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<Vec<Option<ClassSkeleton>>> {
    let code_start = w.pos();

    let mut emitter = CodeEmitter {
        debug: ByteInterner::new()?,
        code_debug_item: Vec::new(),
        scratch: Vec::new(),
        code_buf: Vec::new(),
    };
    let mut layouts: Vec<Option<ClassSkeleton>> = Vec::with_capacity(plan.classes.len());
    let remap = plan.remap();

    for class in &plan.classes {
        let layout = match class {
            WriteClass::Resident(class) => match &class.class_data {
                Some(data) => Some(emitter.write_resident(w, plan, data, remap)?),
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
    w.map_section(TYPE_CODE_ITEM, code_start, code_item_count);

    let debug_start = w.pos();
    if !emitter.debug.is_empty() {
        emitter.debug.write_to(&mut w.sink)?;
        w.map_section(
            TYPE_DEBUG_INFO_ITEM,
            debug_start,
            emitter.debug.len() as u32,
        );
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

/// A method as its class lists it, with its body still where it lives.
struct Member<'a> {
    method: MethodIdx,
    access_flags: AccessFlags,
    body: Body<'a>,
}

enum Body<'a> {
    None,
    Resident(&'a CodeItem),
    File(u32),
}

impl Member<'_> {
    fn resident(methods: &[EncodedMethod]) -> Vec<Member<'_>> {
        methods
            .iter()
            .map(|method| Member {
                method: method.method,
                access_flags: method.access_flags,
                body: method.code().map_or(Body::None, Body::Resident),
            })
            .collect()
    }

    fn file(headers: Vec<MethodHeader>) -> Vec<Member<'static>> {
        headers
            .into_iter()
            .map(|header| Member {
                method: header.method,
                access_flags: header.access_flags,
                body: match header.code_off {
                    0 => Body::None,
                    code_off => Body::File(code_off),
                },
            })
            .collect()
    }
}

impl CodeEmitter {
    fn write_resident<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        data: &ClassData,
        remap: Option<&Remap>,
    ) -> Result<ClassSkeleton> {
        self.write_class(
            w,
            plan,
            [data.static_fields.clone(), data.instance_fields.clone()],
            [
                Member::resident(&data.direct_methods),
                Member::resident(&data.virtual_methods),
            ],
            remap,
        )
    }

    fn write_deferred<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        offset: u32,
        remap: Option<&Remap>,
    ) -> Result<ClassSkeleton> {
        let skeleton =
            read_class_skeleton_at(plan.raw_bytes(), offset as usize, plan.dex.parse_options)?;
        crate::references::validate_skeleton(plan.dex, &skeleton)?;
        self.write_class(
            w,
            plan,
            [skeleton.static_fields, skeleton.instance_fields],
            [
                Member::file(skeleton.direct_methods),
                Member::file(skeleton.virtual_methods),
            ],
            remap,
        )
    }

    /// Writes the bodies of a class's methods in the member order its class
    /// data lists, which is ascending by output index.
    fn write_class<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        mut fields: [Vec<EncodedField>; 2],
        mut methods: [Vec<Member<'_>>; 2],
        remap: Option<&Remap>,
    ) -> Result<ClassSkeleton> {
        if let Some(remap) = remap {
            for fields in &mut fields {
                for field in fields.iter_mut() {
                    field.field = remap.remap_field(field.field);
                }
                fields.sort_by_key(|field| field.field);
            }
            for methods in &mut methods {
                for member in methods.iter_mut() {
                    member.method = remap.remap_method(member.method);
                }
                methods.sort_by_key(|member| member.method);
            }
        }
        let [static_fields, instance_fields] = fields;
        let [direct, virtual_] = methods;
        Ok(ClassSkeleton {
            static_fields,
            instance_fields,
            direct_methods: self.write_bodies(w, plan, direct, remap)?,
            virtual_methods: self.write_bodies(w, plan, virtual_, remap)?,
        })
    }

    fn write_bodies<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        members: Vec<Member<'_>>,
        remap: Option<&Remap>,
    ) -> Result<Vec<MethodHeader>> {
        members
            .into_iter()
            .map(|member| {
                let code_off = match member.body {
                    Body::None => 0,
                    Body::Resident(code) if remap.is_none() => self.write_code(w, code)?,
                    Body::Resident(code) => self.write_owned(w, code.clone(), remap)?,
                    Body::File(code_off) => self.write_file_code(w, plan, code_off, remap)?,
                };
                Ok(MethodHeader {
                    method: member.method,
                    access_flags: member.access_flags,
                    code_off,
                })
            })
            .collect()
    }

    fn write_file_code<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        plan: &WritePlan<'_>,
        code_off: u32,
        remap: Option<&Remap>,
    ) -> Result<u32> {
        let source = plan.dex.raw.as_ref().expect("file code retains its source");
        let buf = source.as_bytes();
        let opts = plan.dex.parse_options;
        self.code_buf.clear();
        if let RawCopy::Copied = copy_code_item(buf, code_off, remap, opts, &mut self.code_buf)? {
            w.align(4);
            let off = w.pos();
            w.write(&self.code_buf);
            let debug_off = reseam_bytes::u32_at(buf, code_off as usize + 8);
            let item = if debug_off != 0 && w.options.debug_info == MetadataPolicy::Preserve {
                self.scratch.clear();
                copy_debug_info(buf, debug_off, remap, opts, &mut self.scratch)?;
                Some(self.debug.intern(&self.scratch)? as u32)
            } else {
                None
            };
            self.code_debug_item.push((off, item));
            return Ok(off);
        }
        self.write_owned(w, read_code_item(source, code_off, opts)?, remap)
    }

    /// Writes a decoded body, first dropping debug info the policy omits and
    /// moving its references to the output pools.
    fn write_owned<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        mut code: CodeItem,
        remap: Option<&Remap>,
    ) -> Result<u32> {
        if code
            .debug_info
            .as_ref()
            .is_some_and(|metadata| !metadata.kept_by(w.options.debug_info, w.original.as_ref()))
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
            .filter(|metadata| metadata.kept_by(w.options.debug_info, w.original.as_ref()))
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
