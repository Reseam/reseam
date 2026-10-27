// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{Loading, ParseOptions};

#[test]
fn invalid_descriptors_leave_type_and_string_pools_unchanged() {
    let mut dex = common::dex();
    let types = dex.types().len();
    let strings = dex.strings().len();
    for descriptor in ["", "L;", "[V", "Lmissing", "II"] {
        assert!(dex.intern_type(descriptor).is_err());
        assert_eq!(dex.types().len(), types);
        assert_eq!(dex.strings().len(), strings);
    }
}

#[test]
fn call_sites_and_method_handles_survive_deferred_reads_appends_and_remapping() -> TestResult {
    use reseam_dex::{CallSiteItem, EncodedValue, Instruction, MethodHandle};
    use reseam_dex::{MethodHandleMember, MethodHandleType};
    let mut dex = common::dex();
    let bootstrap = dex.intern_method("Lexample/Bootstrap;", "bootstrap", "()V")?;
    let handle = dex.intern_method_handle(
        MethodHandle {
            handle_type: MethodHandleType::InvokeStatic,
            member: MethodHandleMember::Method(bootstrap),
        },
        None,
    )?;
    let site = CallSiteItem {
        bootstrap_method: handle,
        method_name: dex.intern_string("dynamicCall"),
        method_type: dex.intern_proto("()V")?,
        extra_arguments: vec![EncodedValue::String(dex.intern_string("argument"))],
    };
    let index = dex.intern_call_site(site, None)?;
    common::body(&mut dex).set_instructions(vec![
        Instruction::InvokeCustom {
            call_site: index,
            args: reseam_dex::RegList::default(),
        },
        Instruction::ReturnVoid,
    ]);
    let input = common::write(&dex)?;
    let mut parsed = common::parse_with_loading(&input, Loading::Deferred)?;
    assert!(parsed.call_site(reseam_dex::CallSiteIdx(1)).is_err());
    let mut appended = parsed.call_site(reseam_dex::CallSiteIdx(0))?.into_owned();
    appended.method_name = parsed.intern_string("!appendedCall");
    parsed.intern_call_site(appended, None)?;
    let bytes = common::write(&parsed)?;
    let result = reseam_dex::parse(&bytes, ParseOptions::default())?;
    assert_eq!(result.call_sites().len(), 2);
    for (index, name) in [(0, "dynamicCall"), (1, "!appendedCall")] {
        let site = result.call_site(reseam_dex::CallSiteIdx(index))?;
        assert_eq!(result.strings().get(site.method_name), name);
        let EncodedValue::String(argument) = site.extra_arguments[0] else {
            panic!("string argument");
        };
        assert_eq!(result.strings().get(argument), "argument");
        let handle = result.method_handle(site.bootstrap_method)?;
        let MethodHandleMember::Method(target) = handle.member else {
            panic!("bootstrap method");
        };
        assert_eq!(
            result.strings().get(result.method_id(target).name),
            "bootstrap"
        );
    }
    Ok(())
}

#[test]
fn invalid_pool_references_fail_before_lookup_or_serialization() {
    use reseam_dex::{Instruction, MethodIdx, StringIdx};
    let mut dex = common::dex();
    let method = common::direct_method(&mut dex);
    method.method = MethodIdx(u32::MAX);
    assert!(dex.find_methods_with_opcodes(&[]).is_err());
    assert!(common::write(&dex).is_err());
    let mut dex = common::dex();
    common::body(&mut dex).set_instructions(vec![
        Instruction::ConstString {
            dest: 0,
            string: StringIdx(u32::MAX),
        },
        Instruction::ReturnVoid,
    ]);
    assert!(reseam_dex::write_spooled(&dex, None).is_err());
}
