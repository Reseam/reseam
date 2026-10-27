// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Read;

use reseam_dex::{DexFile, DexHeader, DexVersion, Spooled};

pub fn dex_bytes(dex: &DexFile) -> Vec<u8> {
    spooled_bytes(&reseam_dex::write_spooled(dex, None).expect("DEX fixture").0)
}

pub fn empty_dex() -> DexFile {
    DexFile::new(DexHeader::new(DexVersion::V035))
}

pub fn minimal_dex_bytes() -> Vec<u8> {
    dex_bytes(&empty_dex())
}

pub fn spooled_bytes(spooled: &Spooled) -> Vec<u8> {
    let mut bytes = Vec::new();
    spooled
        .reader()
        .read_to_end(&mut bytes)
        .expect("spooled DEX fixture");
    bytes
}

/// A v041 container of one-class members.
pub fn container_dex_bytes(classes: &[&str]) -> Vec<u8> {
    let members: Vec<_> = classes
        .iter()
        .map(|name| {
            let mut dex = empty_dex();
            dex.create_class(name, reseam_dex::AccessFlags::PUBLIC, None)
                .expect("fixture class");
            dex
        })
        .collect();
    let (spooled, _) = reseam_dex::write_container_spooled(members.iter().map(|dex| (dex, None)))
        .expect("DEX container fixture");
    spooled_bytes(&spooled)
}
