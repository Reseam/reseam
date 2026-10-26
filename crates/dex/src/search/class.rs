// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::ControlFlow;

use super::view::{Code, MethodView};
use crate::error::Result;
use crate::file::DexFile;
use crate::model::TypeIdx;
use crate::model::class::{ClassData, EncodedMethod};
use crate::options::ParseOptions;
use crate::read::class::ClassDataCursor;
use crate::read::code::read_code_item;
use crate::search::index::Slot;

impl DexFile {
    pub(super) fn scan_class<T>(
        &self,
        class_idx: usize,
        methods: Option<&[Slot]>,
        visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        let class_type = self.classes.header(class_idx).class_type;
        if let Some(class) = self.classes.resident(class_idx) {
            return match &class.class_data {
                Some(data) => {
                    scan_resolved_class(class_idx, class_type, data, methods, &mut |view| {
                        crate::references::check_index(
                            self,
                            crate::model::Pool::Method,
                            view.method.0,
                        )?;
                        visit(view)
                    })
                }
                None => Ok(ControlFlow::Continue(())),
            };
        }
        match self.classes.class_data_offset(class_idx) {
            Some(offset) => self.scan_raw_class(class_idx, class_type, offset, methods, visit),
            None => Ok(ControlFlow::Continue(())),
        }
    }

    fn scan_raw_class<T>(
        &self,
        class_idx: usize,
        class_type: TypeIdx,
        offset: u32,
        methods: Option<&[Slot]>,
        visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
    ) -> Result<ControlFlow<T>> {
        let buf = self.raw_bytes(offset)?;
        let opts = self.parse_options;

        let mut cursor = ClassDataCursor::new(buf, offset as usize, opts)?;
        cursor.skip_fields()?;
        for (kind, count) in [
            (
                crate::model::class::MethodKind::Direct,
                cursor.counts.direct_methods,
            ),
            (
                crate::model::class::MethodKind::Virtual,
                cursor.counts.virtual_methods,
            ),
        ] {
            let slot_base = if kind == crate::model::class::MethodKind::Virtual {
                cursor.counts.direct_methods as usize
            } else {
                0
            };
            let mut method_pos = 0;
            let result = cursor.methods(count, |header| {
                crate::references::check_index(self, crate::model::Pool::Method, header.method.0)?;
                let position = method_pos;
                method_pos += 1;
                let slot = Slot::try_from(slot_base + position).map_err(|_| {
                    crate::error::invalid("class data", "a class defines more than 65536 methods")
                })?;
                if methods.is_some_and(|methods| methods.binary_search(&slot).is_err()) {
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
    methods: Option<&[Slot]>,
    visit: &mut impl FnMut(&MethodView<'_>) -> Result<ControlFlow<T>>,
) -> Result<ControlFlow<T>> {
    let lists = [
        (
            crate::model::class::MethodKind::Direct,
            data.direct_methods.as_slice(),
        ),
        (
            crate::model::class::MethodKind::Virtual,
            data.virtual_methods.as_slice(),
        ),
    ];
    let mut slot = 0usize;
    for (kind, list) in lists {
        for (method_pos, method) in list.iter().enumerate() {
            let listed = methods.is_none_or(|methods| {
                Slot::try_from(slot).is_ok_and(|slot| methods.binary_search(&slot).is_ok())
            });
            slot += 1;
            if !listed {
                continue;
            }
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

pub(super) fn decode_one_method(
    source: &crate::DexBytes,
    offset: u32,
    method_pos: usize,
    kind: crate::model::class::MethodKind,
    opts: ParseOptions,
) -> Result<Option<EncodedMethod>> {
    let mut cursor = ClassDataCursor::new(source.as_bytes(), offset as usize, opts)?;
    cursor.skip_fields()?;
    if kind == crate::model::class::MethodKind::Virtual {
        let _: ControlFlow<()> = cursor.methods::<()>(cursor.counts.direct_methods, |_| {
            Ok(ControlFlow::Continue(()))
        })?;
    }
    let count = if kind == crate::model::class::MethodKind::Virtual {
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
            code: code.map(std::sync::Arc::new),
        }))
    })?;
    Ok(result.break_value())
}
