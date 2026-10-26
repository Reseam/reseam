// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;

use super::{StringPool, hash_str};
use crate::encoding::mutf8::encode_mutf8;
use crate::encoding::mutf8::{ascii_order, mutf8_compare, mutf8_units};
use crate::model::StringIdx;

impl StringPool {
    pub fn find(&self, s: &str) -> Option<StringIdx> {
        let bmp = s.chars().all(|c| (c as u32) < 0x10000);
        let target = (!s.contains('\0')).then_some(s.as_bytes());
        let mut encoded = None;
        let mut lo = 0usize;
        let mut hi = self.sorted_len;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let ascii = target
                .zip(self.raw_payload(mid))
                .and_then(|(target, raw)| ascii_order(raw, target));
            let ord = ascii.unwrap_or_else(|| match self.plain(mid) {
                Some(bytes) if bmp => bytes.cmp(s.as_bytes()),
                _ => mutf8_compare(
                    &self.mutf8(mid),
                    encoded.get_or_insert_with(|| encode_mutf8(s)),
                ),
            });
            match ord {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(StringIdx(mid as u32)),
            }
        }
        self.tail
            .get(&hash_str(s))?
            .iter()
            .copied()
            .find(|&i| mutf8_units(&self.mutf8(i as usize)).eq(s.encode_utf16()))
            .map(StringIdx)
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
        let start = self.sorted_partition_point(|i| locate(i).0 == Ordering::Less);
        let end = self.sorted_partition_point(|i| {
            let (order, starts) = locate(i);
            order == Ordering::Less || starts
        });
        (start..end)
            .chain((self.sorted_len..self.len()).filter(|&i| locate(i).1))
            .map(|i| StringIdx(i as u32))
            .collect()
    }

    /// The first index of the sorted prefix for which `before` is false;
    /// `before` must hold for a leading run of it.
    fn sorted_partition_point(&self, before: impl Fn(usize) -> bool) -> usize {
        let (mut lo, mut hi) = (0, self.sorted_len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if before(mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }
}
