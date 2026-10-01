// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_dex::{DexFile, DexHeader, DexVersion};

pub fn minimal_dex_bytes() -> Vec<u8> {
    reseam_dex::write(&DexFile::new(DexHeader::new(DexVersion::V035))).expect("DEX fixture")
}

pub fn container_dex_bytes(classes: &[&str]) -> Vec<u8> {
    let members: Vec<_> = classes
        .iter()
        .map(|name| {
            let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
            dex.create_class(name, reseam_dex::AccessFlags::PUBLIC, None)
                .expect("fixture class");
            dex
        })
        .collect();
    reseam_dex::write_container(&members).expect("DEX container fixture")
}
