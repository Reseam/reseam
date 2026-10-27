// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;

use super::DexPart;
use crate::file::DexFile;
use crate::model::{FieldId, MethodId, PerPool, Pool, StringIdx, TypeIdx};
use crate::write::remap::Remap;

/// The order a DEX's pools are written in and how each source index maps to
/// it; `None` when every pool is written whole and already in order.
pub(super) fn order_pools(
    dex: &DexFile,
    part: Option<&DexPart>,
) -> Option<(PerPool<Vec<u32>>, Remap)> {
    let compact = part.is_some();
    let candidates = |pool| pool_candidates(dex, part, pool);

    let strings = ordered_strings(dex, part);
    let string_remap = strings.as_deref().map(|o| remap_of(o, dex.strings.len()));
    let map_string = |i: StringIdx| string_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

    let types = (compact || !dex.types.is_sorted() || string_remap.is_some()).then(|| {
        merge_sorted(&candidates(Pool::Type), dex.types.sorted_len(), |a, b| {
            map_string(dex.types.get(a as usize)).cmp(&map_string(dex.types.get(b as usize)))
        })
    });
    let type_remap = types.as_deref().map(|o| remap_of(o, dex.types.len()));
    let map_type = |i: TypeIdx| type_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

    let protos = (compact || !dex.prototypes.is_sorted() || type_remap.is_some()).then(|| {
        merge_sorted(
            &candidates(Pool::Proto),
            dex.prototypes.sorted_len(),
            |a, b| {
                let pa = dex.prototypes.get(a as usize);
                let pb = dex.prototypes.get(b as usize);
                map_type(pa.return_type)
                    .cmp(&map_type(pb.return_type))
                    .then_with(|| {
                        pa.parameters
                            .iter()
                            .map(|t| map_type(*t))
                            .cmp(pb.parameters.iter().map(|t| map_type(*t)))
                    })
            },
        )
    });
    let proto_remap = protos.as_deref().map(|o| remap_of(o, dex.prototypes.len()));
    let map_proto =
        |i: crate::model::ProtoIdx| proto_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

    let fields =
        (compact || !dex.fields.is_sorted() || type_remap.is_some() || string_remap.is_some())
            .then(|| {
                ordered_pool(dex, part, Pool::Field, dex.fields.sorted_len(), |i| {
                    let f = dex.fields.get(i as usize);
                    FieldId {
                        class: TypeIdx(map_type(f.class)),
                        name: StringIdx(map_string(f.name)),
                        type_: TypeIdx(map_type(f.type_)),
                    }
                })
            });
    let field_remap = fields.as_deref().map(|o| remap_of(o, dex.fields.len()));

    let methods = (compact
        || !dex.methods.is_sorted()
        || type_remap.is_some()
        || string_remap.is_some()
        || proto_remap.is_some())
    .then(|| {
        ordered_pool(dex, part, Pool::Method, dex.methods.sorted_len(), |i| {
            let m = dex.methods.get(i as usize);
            MethodId {
                class: TypeIdx(map_type(m.class)),
                name: StringIdx(map_string(m.name)),
                proto: crate::ProtoIdx(map_proto(m.proto)),
            }
        })
    });
    let method_remap = methods.as_deref().map(|o| remap_of(o, dex.methods.len()));

    let already_sorted = !compact
        && unchanged_pools([
            &string_remap,
            &type_remap,
            &proto_remap,
            &field_remap,
            &method_remap,
        ]);

    if already_sorted {
        return None;
    }
    let mut ordered = [strings, types, protos, fields, methods, None, None];
    let order = PerPool::from_fn(|pool| {
        ordered[pool as usize]
            .take()
            .unwrap_or_else(|| candidates(pool))
    });
    let remap = Remap::new(PerPool::from_fn(|pool| {
        remap_of(&order[pool], dex.pool_len(pool))
    }));
    Some((order, remap))
}

fn remap_of(order: &[u32], len: usize) -> Vec<u32> {
    let mut remap = vec![u32::MAX; len];
    for (new, &old) in order.iter().enumerate() {
        remap[old as usize] = new as u32;
    }
    remap
}

fn is_identity(remap: &[u32]) -> bool {
    remap.iter().enumerate().all(|(i, &v)| v == i as u32)
}

fn pool_candidates(dex: &DexFile, part: Option<&DexPart>, pool: Pool) -> Vec<u32> {
    match part {
        Some(part) => part.pools.members(pool).collect(),
        None => (0..dex.pool_len(pool) as u32).collect(),
    }
}

fn unchanged_pools(maps: [&Option<Vec<u32>>; 5]) -> bool {
    maps.into_iter()
        .all(|map| map.as_deref().is_none_or(is_identity))
}

fn ordered_strings(dex: &DexFile, part: Option<&DexPart>) -> Option<Vec<u32>> {
    (part.is_some() || !dex.strings.is_sorted()).then(|| {
        merge_sorted(
            &pool_candidates(dex, part, Pool::String),
            dex.strings.sorted_len(),
            |a, b| dex.strings.compare(a, b),
        )
    })
}

fn ordered_pool<K: Ord>(
    dex: &DexFile,
    part: Option<&DexPart>,
    pool: Pool,
    sorted_len: usize,
    key: impl Fn(u32) -> K,
) -> Vec<u32> {
    merge_sorted(&pool_candidates(dex, part, pool), sorted_len, |a, b| {
        key(a).cmp(&key(b))
    })
}

/// `candidates`, ascending indices into a pool, sorted by `cmp`, as a stable
/// sort would order them. Indices below `sorted_len` must already be in `cmp`
/// order, which holds because pool remaps keep a source's sorted prefix in
/// order; only the later entries, the ones patches added, are sorted and
/// placed among them.
fn merge_sorted(
    candidates: &[u32],
    sorted_len: usize,
    cmp: impl Fn(u32, u32) -> Ordering,
) -> Vec<u32> {
    let split = candidates.partition_point(|&index| (index as usize) < sorted_len);
    let (mut prefix, tail) = candidates.split_at(split);
    let mut tail = tail.to_vec();
    tail.sort_by(|&a, &b| cmp(a, b));
    let mut order = Vec::with_capacity(candidates.len());
    for index in tail {
        let before = prefix.partition_point(|&entry| cmp(entry, index) != Ordering::Greater);
        order.extend_from_slice(&prefix[..before]);
        order.push(index);
        prefix = &prefix[before..];
    }
    order.extend_from_slice(prefix);
    order
}
