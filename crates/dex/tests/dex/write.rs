// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{Loading, ParseOptions};
use std::io::Read;

#[test]
fn streamed_resident_and_spooled_writers_preserve_the_same_program() -> TestResult {
    let input = common::write(&common::dex())?;
    let mut streamed = reseam_dex::parse(
        &input,
        ParseOptions {
            classes: Loading::Deferred,
            debug_info: Loading::Deferred,
            annotations: Loading::Deferred,
            ..ParseOptions::default()
        },
    )?;
    let mut resident = reseam_dex::parse(&input, ParseOptions::default())?;
    streamed.intern_string("!first sorted string");
    resident.intern_string("!first sorted string");
    let expected = common::write(&resident)?;
    assert_eq!(common::write(&streamed)?, expected);
    let (spool, _) = reseam_dex::write_spooled(&streamed, None)?;
    let mut actual = Vec::new();
    spool.reader().read_to_end(&mut actual)?;
    assert_eq!(actual, expected);
    reseam_dex::parse(&actual, ParseOptions::default())?;
    Ok(())
}

#[test]
fn prototype_identities_survive_global_pool_overflow_and_part_compaction() -> TestResult {
    use reseam_dex::{AccessFlags, EncodedMethod};
    let mut dex = common::dex();
    let mut descriptor = String::new();
    for value in 0..=u16::MAX {
        descriptor.clear();
        descriptor.push('(');
        descriptor.extend((0..16).map(|bit| if value & (1 << bit) == 0 { 'B' } else { 'I' }));
        descriptor.push_str(")V");
        dex.intern_proto(&descriptor)?;
    }
    let prototype = dex.intern_proto(&descriptor)?;
    assert!(prototype.0 > u32::from(u16::MAX));
    let method = dex.intern_method("Lexample/Activity;", "nativeCall", &descriptor)?;
    dex.class_mut(0)?.add_method(EncodedMethod {
        method,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC | AccessFlags::NATIVE,
        code: None,
    });
    assert!(common::write(&dex).is_err());
    let parts = reseam_dex::split_to_fit(&dex)?.expect("fixture");
    assert_eq!(parts.len(), 1);
    let (spool, _) = reseam_dex::write_spooled(&dex, Some(&parts[0]))?;
    let mut bytes = Vec::new();
    spool.reader().read_to_end(&mut bytes)?;
    let parsed = reseam_dex::parse(&bytes, ParseOptions::default())?;
    let name = parsed.strings().find("nativeCall").expect("fixture");
    let method = reseam_dex::MethodIdx(
        parsed
            .methods()
            .iter()
            .position(|method| method.name == name)
            .expect("fixture") as u32,
    );
    assert_eq!(
        parsed.proto_descriptor(&parsed.proto(parsed.method_id(method).proto)),
        descriptor
    );
    Ok(())
}
