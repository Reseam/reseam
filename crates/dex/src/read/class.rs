// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::code::read_code_item;
use super::encoded_value::read_encoded_array_with_opts;
use super::type_list::{read_type_list, validate_type_list};
use crate::encoding::leb128::read_uleb128;
use crate::error::Result;
use crate::model::access_flags::AccessFlags;
use crate::model::class::{
    ClassData, ClassDef, ClassHeader, EncodedField, EncodedMethod, MethodKind, NO_INDEX,
};
use crate::model::{FieldIdx, MethodIdx};
use crate::model::{StringIdx, TypeIdx};
use crate::options::ParseOptions;

/// Decodes one `class_def_item` record into a resident [`ClassDef`].
pub fn read_class_def(
    source: &crate::DexBytes,
    raw: RawClassDef,
    opts: ParseOptions,
) -> Result<ClassDef> {
    let buf = source.as_bytes();
    let header = raw.header();
    let interfaces = if raw.interfaces_off != 0 {
        let off = raw.interfaces_off as usize;
        validate_type_list(buf, off)?;
        read_type_list(buf, off)
    } else {
        crate::model::TypeList::new()
    };
    let annotations = (raw.annotations_off != 0)
        .then(|| {
            crate::model::metadata::Metadata::from_source(
                source,
                raw.annotations_off,
                opts,
                opts.annotations,
            )
            .map(Box::new)
        })
        .transpose()?;
    let class_data = if raw.class_data_off != 0 {
        Some(Box::new(read_class_data(source, raw.class_data_off, opts)?))
    } else {
        None
    };
    let values = if raw.static_values_off != 0 {
        read_encoded_array_with_opts(buf, raw.static_values_off as usize, opts)?.0
    } else {
        Vec::new()
    };
    let fields = class_data
        .as_ref()
        .map_or(&[][..], |data| data.static_fields.as_slice());
    if values.len() > fields.len() {
        return Err(crate::error::invalid(
            "static values",
            "more values than static fields",
        ));
    }
    let static_values = fields
        .iter()
        .zip(values)
        .map(|(field, value)| (field.field, value))
        .collect();
    Ok(ClassDef {
        class_type: header.class_type,
        access_flags: header.access_flags,
        superclass: header.superclass,
        interfaces,
        source_file: header.source_file,
        annotations,
        class_data,
        static_values,
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemberCounts {
    pub direct_methods: u32,
    pub virtual_methods: u32,
    pub static_fields: u32,
    pub instance_fields: u32,
}

impl MemberCounts {
    pub fn methods(&self, kind: MethodKind) -> u32 {
        match kind {
            MethodKind::Direct => self.direct_methods,
            MethodKind::Virtual => self.virtual_methods,
        }
    }
}

pub(crate) struct ClassDataCursor<'a> {
    buf: &'a [u8],
    opts: ParseOptions,
    pos: usize,
    pub counts: MemberCounts,
}

impl<'a> ClassDataCursor<'a> {
    pub(crate) fn new(buf: &'a [u8], off: usize, opts: ParseOptions) -> Result<Self> {
        let mut cursor = Self {
            buf,
            opts,
            pos: off,
            counts: MemberCounts::default(),
        };
        cursor.counts = MemberCounts {
            static_fields: cursor.uleb()?,
            instance_fields: cursor.uleb()?,
            direct_methods: cursor.uleb()?,
            virtual_methods: cursor.uleb()?,
        };
        Ok(cursor)
    }

    fn uleb(&mut self) -> Result<u32> {
        let (value, size) = read_uleb128(self.buf, self.pos, self.opts)?;
        self.pos += size;
        Ok(value)
    }

    pub(crate) fn fields(&mut self, count: u32, mut visit: impl FnMut(EncodedField)) -> Result<()> {
        let mut index = 0u32;
        for _ in 0..count {
            index = index.wrapping_add(self.uleb()?);
            visit(EncodedField {
                field: FieldIdx(index),
                access_flags: AccessFlags::from_bits_retain(self.uleb()?),
            });
        }
        Ok(())
    }

    pub(crate) fn methods<T>(
        &mut self,
        count: u32,
        mut visit: impl FnMut(MethodHeader) -> Result<std::ops::ControlFlow<T>>,
    ) -> Result<std::ops::ControlFlow<T>> {
        let mut index = 0u32;
        for _ in 0..count {
            index = index.wrapping_add(self.uleb()?);
            let header = MethodHeader {
                method: MethodIdx(index),
                access_flags: AccessFlags::from_bits_retain(self.uleb()?),
                code_off: self.uleb()?,
            };
            if let std::ops::ControlFlow::Break(value) = visit(header)? {
                return Ok(std::ops::ControlFlow::Break(value));
            }
        }
        Ok(std::ops::ControlFlow::Continue(()))
    }

    pub(crate) fn skip_fields(&mut self) -> Result<()> {
        self.fields(self.counts.static_fields, |_| {})?;
        self.fields(self.counts.instance_fields, |_| {})
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MethodHeader {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code_off: u32,
}

/// Member lists with method bodies left in the source file.
pub struct ClassSkeleton {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<MethodHeader>,
    pub virtual_methods: Vec<MethodHeader>,
}

impl ClassSkeleton {
    pub fn methods(&self, kind: MethodKind) -> &[MethodHeader] {
        match kind {
            MethodKind::Direct => &self.direct_methods,
            MethodKind::Virtual => &self.virtual_methods,
        }
    }
}

pub fn read_class_skeleton_at(buf: &[u8], off: usize, opts: ParseOptions) -> Result<ClassSkeleton> {
    let mut cursor = ClassDataCursor::new(buf, off, opts)?;
    let mut skeleton = ClassSkeleton {
        static_fields: Vec::new(),
        instance_fields: Vec::new(),
        direct_methods: Vec::new(),
        virtual_methods: Vec::new(),
    };
    cursor.fields(cursor.counts.static_fields, |field| {
        skeleton.static_fields.push(field);
    })?;
    cursor.fields(cursor.counts.instance_fields, |field| {
        skeleton.instance_fields.push(field);
    })?;
    for (count, methods) in [
        (cursor.counts.direct_methods, &mut skeleton.direct_methods),
        (cursor.counts.virtual_methods, &mut skeleton.virtual_methods),
    ] {
        let _: std::ops::ControlFlow<()> = cursor.methods::<()>(count, |method| {
            methods.push(method);
            Ok(std::ops::ControlFlow::Continue(()))
        })?;
    }
    Ok(skeleton)
}

pub fn read_class_data(
    source: &crate::DexBytes,
    off: u32,
    opts: ParseOptions,
) -> Result<ClassData> {
    let skeleton = read_class_skeleton_at(source.as_bytes(), off as usize, opts)?;
    let decode = |headers: Vec<MethodHeader>| {
        headers
            .into_iter()
            .map(|header| {
                Ok(EncodedMethod {
                    method: header.method,
                    access_flags: header.access_flags,
                    code: (header.code_off != 0)
                        .then(|| read_code_item(source, header.code_off, opts))
                        .transpose()?
                        .map(std::sync::Arc::new),
                })
            })
            .collect::<Result<Vec<_>>>()
    };
    Ok(ClassData {
        static_fields: skeleton.static_fields,
        instance_fields: skeleton.instance_fields,
        direct_methods: decode(skeleton.direct_methods)?,
        virtual_methods: decode(skeleton.virtual_methods)?,
    })
}

pub(crate) const RECORD_SIZE: usize = 32;

/// A `class_def_item` as stored in the file.
#[derive(Debug, Clone, Copy)]
pub struct RawClassDef {
    pub class_type: u32,
    pub access_flags: u32,
    pub superclass: u32,
    pub interfaces_off: u32,
    pub source_file: u32,
    pub annotations_off: u32,
    pub class_data_off: u32,
    pub static_values_off: u32,
}

impl RawClassDef {
    pub(crate) fn read(buf: &[u8], off: usize) -> Self {
        let word = |i: usize| {
            let o = off + i * 4;
            u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
        };
        Self {
            class_type: word(0),
            access_flags: word(1),
            superclass: word(2),
            interfaces_off: word(3),
            source_file: word(4),
            annotations_off: word(5),
            class_data_off: word(6),
            static_values_off: word(7),
        }
    }

    pub fn header(&self) -> ClassHeader {
        ClassHeader {
            class_type: TypeIdx(self.class_type),
            access_flags: AccessFlags::from_bits_retain(self.access_flags),
            superclass: (self.superclass != NO_INDEX).then_some(TypeIdx(self.superclass)),
            source_file: (self.source_file != NO_INDEX).then_some(StringIdx(self.source_file)),
        }
    }
}
