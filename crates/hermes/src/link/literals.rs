// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use crate::HermesFile;
use crate::edit::Edits;
use crate::error::{HermesError, Result, invalid};
use crate::layout::Section;
use crate::literal::{Literal, MAX_RUN, read_literals};
use reseam_bytes::u32_at;

/// A linked module's literal buffer, copied per reference. Hermes shares
/// byte-identical literal sequences, so a reference may start inside another
/// run; each one is read from its own offset and length, as the runtime reads
/// it, and copied with its string IDs remapped. Short strings become long
/// strings, since remapped IDs need not fit 16 bits.
pub(super) struct Literals<'a> {
    module: &'a HermesFile<'a>,
    buffer: Section,
    strings: &'a [u32],
    base: u32,
    pub(super) output: Vec<u8>,
    copies: BTreeMap<(u32, u32), u32>,
}

impl<'a> Literals<'a> {
    pub(super) fn new(
        module: &'a HermesFile<'a>,
        buffer: Section,
        strings: &'a [u32],
        base: u32,
    ) -> Self {
        Self {
            module,
            buffer,
            strings,
            base,
            output: Vec::new(),
            copies: BTreeMap::new(),
        }
    }

    /// The offset of the copy of `count` literals read from `offset`.
    pub(super) fn relocate(&mut self, offset: u32, count: u32) -> Result<u32> {
        if let Some(&copy) = self.copies.get(&(offset, count)) {
            return Ok(copy);
        }
        let copy = u32::try_from(self.output.len())
            .ok()
            .and_then(|length| self.base.checked_add(length))
            .ok_or(HermesError::Limit("four GiB of literals"))?;
        let literals = read_literals(self.module, self.buffer, offset as usize, count as usize)?;
        for run in literals.chunk_by(|a, b| a.kind == b.kind) {
            for run in run.chunks(MAX_RUN) {
                let kind = match run[0].kind {
                    Literal::ShortString => Literal::LongString,
                    kind => kind,
                };
                if run.len() > usize::from(Literal::LENGTH) {
                    self.output
                        .push(Literal::LONG_RUN | kind.tag() | (run.len() >> 8) as u8);
                    self.output.push(run.len() as u8);
                } else {
                    self.output.push(kind.tag() | run.len() as u8);
                }
                for literal in run {
                    if let Some(id) = literal.string() {
                        let id = self.strings.get(id.0 as usize).ok_or_else(|| {
                            invalid(offset as usize, "literal string ID out of range")
                        })?;
                        self.output.extend_from_slice(&id.to_le_bytes());
                    } else {
                        self.output.extend_from_slice(literal.bytes);
                    }
                }
            }
        }
        self.copies.insert((offset, count), copy);
        Ok(copy)
    }
}

/// Appends the module's object shapes, with the key literals they read.
pub(super) fn copy_shapes(
    app: &HermesFile<'_>,
    edits: &mut Edits,
    module: &HermesFile<'_>,
    strings: &[u32],
) -> Result<()> {
    let mut keys = Literals::new(
        module,
        Section::Keys,
        strings,
        edits.section_size(app, Section::Keys) as u32,
    );
    for entry in module.section(Section::Shapes).as_chunks::<8>().0 {
        let offset = keys.relocate(u32_at(entry, 0), u32_at(entry, 4))?;
        edits.append(Section::Shapes, &offset.to_le_bytes());
        edits.append(Section::Shapes, &entry[4..]);
    }
    edits.append(Section::Keys, &keys.output);
    Ok(())
}

/// Appends the module's bigints and regexps with their storage.
pub(super) fn copy_storage(
    app: &HermesFile<'_>,
    edits: &mut Edits,
    module: &HermesFile<'_>,
) -> Result<()> {
    for (table, storage) in [
        (Section::BigInts, Section::BigIntStorage),
        (Section::Regexps, Section::RegexpStorage),
    ] {
        let base = edits.section_size(app, storage) as u32;
        for entry in module.section(table).as_chunks::<8>().0 {
            let offset = u32_at(entry, 0)
                .checked_add(base)
                .ok_or(HermesError::Limit("four GiB of literal storage"))?;
            edits.append(table, &offset.to_le_bytes());
            edits.append(table, &entry[4..]);
        }
        edits.append(storage, module.section(storage));
    }
    Ok(())
}
