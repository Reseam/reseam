// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::read_u32;
use std::ops::Range;

use rayon::prelude::*;

use super::postings::Postings;
use super::{PoolKeys, RefIndex, Slot};
use crate::error::{Result, invalid};
use crate::file::ClassTable;
use crate::instruction::stream::walk_instructions;
use crate::options::ParseOptions;
use crate::read::class::read_class_skeleton_at;

/// Classes one parallel task walks.
const CHUNK: usize = 64;

/// Classes walked before their references move into exact-size buffers.
/// Workers hold one batch's references at a time, so the memory they leave
/// with the allocator stays small.
const BATCH: usize = 16 * CHUNK;

/// The references of a run of classes, method positions counted from the
/// chunk's first method.
struct ChunkRefs {
    methods: Vec<u32>,
    pool: Vec<(usize, u32)>,
    literals: Vec<(i64, u32)>,
}

/// Every source method's references, in slot order.
struct Refs {
    class_start: Vec<u32>,
    /// Pool key and slot, packed by [`pack`].
    pool: Vec<Vec<u64>>,
    literals: Vec<Vec<(i64, Slot)>>,
}

/// A key and slot in one integer.
fn pack(key: usize, slot: Slot) -> u64 {
    (key as u64) << 16 | u64::from(slot)
}

fn unpack(packed: u64) -> (usize, Slot) {
    ((packed >> 16) as usize, packed as Slot)
}

impl RefIndex {
    pub(crate) fn build(classes: &ClassTable, keys: PoolKeys, opts: ParseOptions) -> Result<Self> {
        let refs = collect(classes, &keys, opts)?;
        let pools = Postings::counted(
            keys.end,
            refs.pool.iter().flatten().map(|&packed| unpack(packed)),
        );
        let mut literal_values: Vec<i64> = refs
            .literals
            .iter()
            .flatten()
            .map(|&(value, _)| value)
            .collect();
        literal_values.par_sort_unstable();
        literal_values.dedup();
        literal_values.shrink_to_fit();
        let literals = Postings::counted(
            literal_values.len(),
            refs.literals.iter().flatten().map(|&(value, slot)| {
                let key = literal_values
                    .binary_search(&value)
                    .expect("every literal value was collected");
                (key, slot)
            }),
        );
        Ok(Self {
            class_start: refs.class_start,
            uncovered: classes.resident_indices().to_vec(),
            keys,
            pools,
            literal_values,
            literals,
        })
    }
}

fn collect(table: &ClassTable, keys: &PoolKeys, opts: ParseOptions) -> Result<Refs> {
    let classes = table.len();
    let mut refs = Refs {
        class_start: Vec::with_capacity(classes + 1),
        pool: Vec::new(),
        literals: Vec::new(),
    };
    let mut next = 0u32;
    for batch in (0..classes).step_by(BATCH) {
        let chunks: Vec<ChunkRefs> = (batch..(batch + BATCH).min(classes))
            .step_by(CHUNK)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|start| chunk_refs(table, keys, opts, start..(start + CHUNK).min(classes)))
            .collect::<Result<_>>()?;
        let mut pool = Vec::with_capacity(chunks.iter().map(|chunk| chunk.pool.len()).sum());
        let mut literals =
            Vec::with_capacity(chunks.iter().map(|chunk| chunk.literals.len()).sum());
        for chunk in chunks {
            let methods: u32 = chunk.methods.iter().sum();
            if next + methods > u32::from(Slot::MAX) + 1 {
                return Err(invalid(
                    "class data",
                    "a DEX defines more than 65536 methods",
                ));
            }
            let slot = |position: u32| {
                Slot::try_from(next + position).expect("the chunk's slots were checked to fit")
            };
            pool.extend(
                chunk
                    .pool
                    .iter()
                    .map(|&(key, position)| pack(key, slot(position))),
            );
            literals.extend(
                chunk
                    .literals
                    .iter()
                    .map(|&(value, position)| (value, slot(position))),
            );
            for count in chunk.methods {
                refs.class_start.push(next);
                next += count;
            }
        }
        refs.pool.push(pool);
        refs.literals.push(literals);
    }
    refs.class_start.push(next);
    Ok(refs)
}

fn chunk_refs(
    table: &ClassTable,
    keys: &PoolKeys,
    opts: ParseOptions,
    classes: Range<usize>,
) -> Result<ChunkRefs> {
    let mut refs = ChunkRefs {
        methods: Vec::with_capacity(classes.len()),
        pool: Vec::new(),
        literals: Vec::new(),
    };
    let mut method = MethodRefs::default();
    let mut position = 0u32;
    for class_idx in classes {
        let Some(offset) = table.class_data_offset(class_idx) else {
            refs.methods.push(0);
            continue;
        };
        let buf = table.raw_bytes();
        let skeleton = read_class_skeleton_at(buf, offset as usize, opts)?;
        let headers = skeleton
            .direct_methods
            .iter()
            .chain(&skeleton.virtual_methods);
        let mut count = 0u32;
        for header in headers {
            if header.code_off != 0 {
                method.collect(buf, header.code_off, keys)?;
                refs.pool
                    .extend(method.pool.iter().map(|&key| (key, position)));
                refs.literals
                    .extend(method.literals.iter().map(|&value| (value, position)));
            }
            position += 1;
            count += 1;
        }
        refs.methods.push(count);
    }
    Ok(refs)
}

/// One method's distinct references, reused across methods.
#[derive(Default)]
struct MethodRefs {
    pool: Vec<usize>,
    literals: Vec<i64>,
}

impl MethodRefs {
    fn collect(&mut self, buf: &[u8], code_off: u32, keys: &PoolKeys) -> Result<()> {
        self.pool.clear();
        self.literals.clear();
        let base = code_off as usize;
        let insns_size = read_u32(buf, base + 12, "code item")? as usize;
        let mut outside = None;
        walk_instructions(buf, base + 16, insns_size, |insn| {
            if let Some((pool, id)) = insn.member_or_string_ref(buf) {
                let Some(key) = keys.pool_key(pool, id) else {
                    outside = Some(id);
                    return false;
                };
                self.pool.push(key);
            } else if let Some(value) = insn.literal(buf) {
                self.literals.push(value);
            }
            true
        })?;
        if let Some(id) = outside {
            return Err(invalid(
                "references",
                format!("reference {id} is outside its pool"),
            ));
        }
        self.pool.sort_unstable();
        self.pool.dedup();
        self.literals.sort_unstable();
        self.literals.dedup();
        Ok(())
    }
}
