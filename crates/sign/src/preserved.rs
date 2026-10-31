// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::block::{self, BLOCK_ID_PADDING, BLOCK_ID_V2, BLOCK_ID_V3};
use crate::error::Result;
use crate::zip_layout;

const BLOCK_ID_V3_1: u32 = 0x1b93_ad61;
const BLOCK_ID_SOURCE_STAMP_V1: u32 = 0x2b09_189e;
const BLOCK_ID_SOURCE_STAMP_V2: u32 = 0x6dff_800d;

/// Pairs that sign the old contents, which a new signature invalidates, and
/// padding, which the signer sizes again.
const REPLACED: [u32; 6] = [
    BLOCK_ID_V2,
    BLOCK_ID_V3,
    BLOCK_ID_V3_1,
    BLOCK_ID_SOURCE_STAMP_V1,
    BLOCK_ID_SOURCE_STAMP_V2,
    BLOCK_ID_PADDING,
];

/// The pairs of an APK's signing block that stay valid under a new signature,
/// such as Play's frosting, which apps can read back at runtime.
pub struct PreservedPairs {
    encoded: Vec<u8>,
}

impl PreservedPairs {
    /// Collects the preserved pairs of `apk`'s signing block in block order.
    /// An APK without a signing block has none.
    pub fn from_apk(apk: &[u8]) -> Result<Self> {
        let range = zip_layout::sections(apk)?.signing_block();
        let mut encoded = Vec::new();
        if !range.is_empty() {
            for pair in block::pairs(&apk[range.start as usize..range.end as usize]) {
                let pair = pair?;
                if !REPLACED.contains(&pair.id) {
                    encoded.extend_from_slice(pair.encoded);
                }
            }
        }
        Ok(Self { encoded })
    }

    pub(crate) fn encoded(&self) -> &[u8] {
        &self.encoded
    }
}
