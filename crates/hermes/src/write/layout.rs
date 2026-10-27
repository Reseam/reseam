// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use crate::FunctionId;
use crate::HermesFile;
use crate::bytes::align;
use crate::edit::{EditedFunction, Edits};
use crate::error::{HermesError, Result};
use crate::function::{ExceptionHandler, FunctionHeader, LARGE_HEADER_SIZE, SMALL_HEADER_SIZE};
use crate::layout::{FOOTER_SIZE, Field, HEADER_SIZE, Section};

/// Where the written file puts everything that moves. Sections keep their order and grow by their
/// additions; the metadata after them shifts as a whole; edited bodies, then new large headers,
/// follow it, and the debug information moves after those.
pub(super) struct Layout {
    pub header: [u8; HEADER_SIZE],
    pub functions: Vec<Placed>,
    /// The relocated body offset each original large header holds, by where that field is.
    pub patches: BTreeMap<usize, [u8; 4]>,
}

/// A function's header as written, and where its large header is.
pub(super) struct Placed {
    pub header: FunctionHeader,
    pub large: Option<u32>,
    /// Whether the large header is written after the bodies, rather than kept in the metadata.
    pub emits_large: bool,
}

impl Layout {
    /// Lays out `functions`, each function's edited form by ID, or `None` where it is unchanged.
    pub(super) fn plan(
        file: &HermesFile<'_>,
        edits: &Edits,
        functions: &[Option<&EditedFunction>],
    ) -> Result<Self> {
        let old_tail = file.sections[Section::Sources as usize].end;
        let old_debug = file.field(Field::DebugInfoOffset) as usize;
        let mut cursor = HEADER_SIZE + functions.len() * SMALL_HEADER_SIZE;
        for section in &Section::ALL[1..] {
            cursor = align(cursor) + edits.section_size(file, *section);
        }
        let shift = cursor.checked_sub(old_tail).expect("sections only grow");
        cursor += old_debug - old_tail;
        let mut placed = Vec::with_capacity(functions.len());
        let mut patches = BTreeMap::new();
        for (index, edited) in functions.iter().enumerate() {
            let original = (index < file.function_count() as usize)
                .then(|| file.function(FunctionId(index as u32)))
                .transpose()?;
            if let Some(original) = &original
                && let Some(large) = original.header.large
            {
                let offset = relocate(original.header.offset as usize, shift)?;
                patches.insert(large, offset.to_le_bytes());
            }
            placed.push(if let Some(edited) = edited {
                let mut header = edited.header.clone();
                cursor = edited.body.place(cursor);
                header.offset = as_offset(cursor)?;
                cursor += edited.body.len();
                Placed {
                    header,
                    large: None,
                    emits_large: true,
                }
            } else {
                let mut header = original.expect("only app functions are unchanged").header;
                header.offset = relocate(header.offset as usize, shift)?;
                let large = header
                    .large
                    .map(|offset| relocate(offset, shift))
                    .transpose()?;
                Placed {
                    emits_large: large.is_none() && !header.fits_small(),
                    header,
                    large,
                }
            });
        }
        for (function, edited) in placed.iter_mut().zip(functions) {
            if !function.emits_large {
                continue;
            }
            cursor = align(cursor);
            function.large = Some(as_offset(cursor)?);
            cursor = align(cursor + LARGE_HEADER_SIZE);
            if let Some(edited) = edited
                && !edited.exceptions.is_empty()
            {
                cursor += ExceptionHandler::table_size(edited.exceptions.len());
            }
        }
        let debug = as_offset(align(cursor))?;
        let old_footer = file.field(Field::FileLength) as usize - FOOTER_SIZE;
        let length = as_offset(align(cursor) + old_footer - old_debug + FOOTER_SIZE)?;
        let mut header: [u8; HEADER_SIZE] = file.source[..HEADER_SIZE]
            .try_into()
            .expect("parsing checked the header size");
        let mut put = |field: Field, value: u32| {
            header[field as usize..field as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(Field::FileLength, length);
        put(Field::GlobalFunction, edits.global(file).0);
        put(Field::FunctionCount, functions.len() as u32);
        for section in &Section::ALL[1..] {
            put(section.count(), edits.section_count(file, *section));
        }
        put(Field::DebugInfoOffset, debug);
        put(Field::StringSwitchCount, edits.string_switches());
        Ok(Self {
            header,
            functions: placed,
            patches,
        })
    }
}

fn as_offset(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| HermesError::Limit("four GiB"))
}

fn relocate(offset: usize, shift: usize) -> Result<u32> {
    as_offset(offset.saturating_add(shift))
}
