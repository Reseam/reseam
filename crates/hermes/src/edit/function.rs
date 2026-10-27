// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::error::Result;
use crate::file::HermesFile;
use crate::function::{
    ExceptionHandler, Function, FunctionFlags, FunctionHeader, FunctionKind, Prohibit,
};
use crate::string::StringId;
use crate::switch::switch_table;

#[derive(Clone)]
pub(crate) struct EditedFunction {
    pub header: FunctionHeader,
    pub body: FunctionBody,
    pub exceptions: Vec<ExceptionHandler>,
}

impl EditedFunction {
    /// Edited functions are written with a new large header and no debug info.
    pub(crate) fn new(
        mut header: FunctionHeader,
        body: FunctionBody,
        exceptions: Vec<ExceptionHandler>,
    ) -> Self {
        header.flags.debug_info = false;
        header.flags.exception_handler = !exceptions.is_empty();
        header.large = None;
        Self {
            header,
            body,
            exceptions,
        }
    }
}

#[derive(Clone)]
pub(crate) enum FunctionBody {
    Original(Range<usize>),
    Owned(Vec<u8>),
}

impl FunctionBody {
    pub(crate) fn bytes<'a>(&'a self, file: &'a HermesFile<'_>) -> &'a [u8] {
        match self {
            Self::Original(range) => &file.source[range.clone()],
            Self::Owned(bytes) => bytes,
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Original(range) => range.len(),
            Self::Owned(bytes) => bytes.len(),
        }
    }
    pub(crate) fn place(&self, cursor: usize) -> usize {
        let remainder = match self {
            Self::Original(range) => range.start & 3,
            Self::Owned(_) => 0,
        };
        cursor + ((remainder + 4 - (cursor & 3)) & 3)
    }
}

/// A strict function that refuses construction, as compiled for ordinary strict-mode code.
pub(crate) fn generated_function(
    name: StringId,
    parameters: u32,
    frame_size: u32,
    body: Vec<u8>,
) -> EditedFunction {
    EditedFunction::new(
        FunctionHeader {
            offset: 0,
            parameters,
            loop_depth: 0,
            size: body.len() as u32,
            name: name.0,
            number_regs: 0,
            non_pointer_regs: 0,
            frame_size,
            read_cache: 0,
            write_cache: 0,
            object_cache: 0,
            private_cache: 0,
            flags: FunctionFlags {
                prohibit: Prohibit::Construct,
                strict: true,
                exception_handler: false,
                debug_info: false,
                kind: FunctionKind::Normal,
            },
            large: None,
        },
        FunctionBody::Owned(body),
        Vec::new(),
    )
}

/// `function` unchanged, its body still read from the source with the switch tables after it.
pub(crate) fn original_function(function: &Function<'_>) -> Result<EditedFunction> {
    let mut end = function.body_range().end;
    for inst in function.instructions()? {
        if let Some(table) = switch_table(function, &inst)? {
            end = end.max(table.offset + table.bytes.len());
        }
    }
    Ok(EditedFunction::new(
        function.header.clone(),
        FunctionBody::Original(function.body_range().start..end),
        function.exception_handlers()?,
    ))
}
