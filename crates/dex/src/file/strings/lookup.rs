// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;

use super::{StringPool, hash_str};
use crate::encoding::mutf8::{ascii_order, encode_mutf8, mutf8_compare, mutf8_units};
use crate::model::StringIdx;

impl StringPool {
    pub fn find(&self, s: &str) -> Option<StringIdx> {
        let bmp = s.chars().all(|c| (c as u32) < 0x10000);
        let target = (!s.contains('\0')).then_some(s.as_bytes());
        let encoded = std::cell::OnceCell::new();
        self.index
            .search(|i| {
                target
                    .zip(self.raw_payload(i))
                    .and_then(|(target, raw)| ascii_order(raw, target))
                    .unwrap_or_else(|| match self.plain(i) {
                        Some(bytes) if bmp => bytes.cmp(s.as_bytes()),
                        _ => mutf8_compare(&self.mutf8(i), encoded.get_or_init(|| encode_mutf8(s))),
                    })
            })
            .or_else(|| {
                self.index
                    .appended(hash_str(s))
                    .find(|&i| mutf8_units(&self.mutf8(i)).eq(s.encode_utf16()))
            })
            .map(|i| StringIdx(i as u32))
    }

    /// Every entry that starts with `prefix`, in index order: a range of the
    /// sorted prefix, then the later entries that match.
    pub fn with_prefix(&self, prefix: &str) -> Vec<StringIdx> {
        let encoded = encode_mutf8(prefix);
        let bmp = prefix.chars().all(|c| (c as u32) < 0x10000);
        // How entry `i` orders against `prefix`, and whether it starts with it.
        let locate = |i: usize| match self.plain(i) {
            Some(bytes) if bmp => (
                bytes.cmp(prefix.as_bytes()),
                bytes.starts_with(prefix.as_bytes()),
            ),
            _ => {
                let entry = self.mutf8(i);
                (mutf8_compare(&entry, &encoded), entry.starts_with(&encoded))
            }
        };
        let start = self
            .index
            .partition_point(|i| locate(i).0 == Ordering::Less);
        let end = self.index.partition_point(|i| {
            let (order, starts) = locate(i);
            order == Ordering::Less || starts
        });
        (start..end)
            .chain((self.index.sorted_len()..self.len()).filter(|&i| locate(i).1))
            .map(|i| StringIdx(i as u32))
            .collect()
    }
}
