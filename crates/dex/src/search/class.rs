// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::ControlFlow;

use super::view::{Code, MethodView};
use crate::error::Result;
use crate::file::DexFile;
use crate::model::TypeIdx;
use crate::model::class::{ClassData, EncodedMethod, MethodKind};
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
        let counts = cursor.counts;
        let mut slot_base = 0;
        for kind in MethodKind::ALL {
            let mut method_pos = 0;
            let result = cursor.methods(counts.methods(kind), |header| {
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
            slot_base += counts.methods(kind) as usize;
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
    let mut slot = 0usize;
    for kind in MethodKind::ALL {
        for (method_pos, method) in data.methods(kind).iter().enumerate() {
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
    kind: MethodKind,
    opts: ParseOptions,
) -> Result<Option<EncodedMethod>> {
    let mut cursor = ClassDataCursor::new(source.as_bytes(), offset as usize, opts)?;
    cursor.skip_fields()?;
    if kind == MethodKind::Virtual {
        let _: ControlFlow<()> = cursor.methods::<()>(cursor.counts.direct_methods, |_| {
            Ok(ControlFlow::Continue(()))
        })?;
    }
    let mut position = 0;
    let result = cursor.methods(cursor.counts.methods(kind), |header| {
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
