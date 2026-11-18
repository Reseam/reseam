// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{
    AccessFlags, ClassImport, DexFile, DexHeader, DexVersion, ImportedPools, Loading, write_class,
};

fn host() -> DexFile {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V039));
    dex.create_class(
        "Lexample/Host;",
        AccessFlags::PUBLIC,
        Some("Ljava/lang/Object;"),
    )
    .expect("valid fixture class");
    dex
}

#[test]
fn an_imported_class_is_defined_as_its_source_defines_it() -> TestResult {
    for loading in [Loading::Eager, Loading::Deferred] {
        let source = common::parse_with_loading(&common::write(&common::dex())?, loading)?;
        let mut host = host();
        let mut pools = ImportedPools::none(&source);
        assert_eq!(
            host.import_class(&source, 0, &mut pools)?,
            ClassImport::Imported(1)
        );
        let written = common::parse_with_loading(&common::write(&host)?, loading)?;
        let class = written
            .find_class_index("Lexample/Activity;")
            .expect("the imported class");
        assert_eq!(write_class(&written, class)?, write_class(&source, 0)?);
    }
    Ok(())
}

#[test]
fn a_class_whose_ids_do_not_fit_leaves_the_dex_as_it_was() -> TestResult {
    let mut host = host();
    let mut descriptor = String::new();
    for value in 0..=u16::MAX {
        descriptor.clear();
        descriptor.push('(');
        descriptor.extend((0..16).map(|bit| if value & (1 << bit) == 0 { 'B' } else { 'I' }));
        descriptor.push_str(")V");
        host.intern_proto(&descriptor)?;
    }
    assert_eq!(host.id_headroom(), 0);
    let before = (
        host.strings().len(),
        host.types().len(),
        host.methods().len(),
    );
    let source = common::dex();
    let mut pools = ImportedPools::none(&source);
    assert_eq!(
        host.import_class(&source, 0, &mut pools)?,
        ClassImport::Full
    );
    assert_eq!(
        (
            host.strings().len(),
            host.types().len(),
            host.methods().len()
        ),
        before
    );
    assert_eq!(host.classes().len(), 1);
    Ok(())
}
