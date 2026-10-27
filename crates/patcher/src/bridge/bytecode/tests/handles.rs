// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

#[test]
fn handles_from_finished_runs_cannot_alias_current_objects() {
    enum Thread {
        Current,
        Other,
    }
    for thread in [Thread::Current, Thread::Other] {
        let previous = match thread {
            Thread::Current => previous_run_handles(),
            Thread::Other => std::thread::spawn(previous_run_handles).join().unwrap(),
        };
        let (directory, mut apk) = crate::test_support::apk();
        apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
        let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        let current = example_handles("New");
        assert!(lookup::get_class_info(previous.class).is_none());
        assert!(lookup::get_method_info(previous.method).is_none());
        assert!(crate::bridge::apk::xml::xml_tag_name(previous.document, previous.root).is_empty());
        assert_eq!(
            lookup::get_class_info(current.class).unwrap().descriptor,
            "LNew;"
        );
        assert_eq!(
            lookup::get_method_info(current.method)
                .unwrap()
                .class_descriptor,
            "LNew;"
        );
        assert_eq!(
            crate::bridge::apk::xml::xml_tag_name(current.document, current.root),
            "New"
        );
        assert!(matches!(
            guard.finish(),
            Err(crate::error::PatcherError::Bridge(_))
        ));
    }
}

#[test]
fn handles_survive_class_removal_and_callback_boundaries() {
    let (dir, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut ctx, RunningBundle::at(dir.path().to_owned())).unwrap();
    let classes: Vec<_> = ["LA;", "LB;", "LC;"]
        .into_iter()
        .map(|name| {
            let class = fixture_class(0, name);
            let methods: Vec<_> = ["first", "second"]
                .into_iter()
                .map(|method| {
                    methods::add_method(
                        class,
                        NewMethod {
                            instructions: vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
                            ..fixture_method(method.into(), "()V".into(), 0, 0)
                        },
                    )
                })
                .collect();
            (class, methods)
        })
        .collect();
    class_ops::remove_class(classes[1].0);
    for (descriptor, (class, methods)) in ["LA;", "LC;"].into_iter().zip([&classes[0], &classes[2]])
    {
        assert_eq!(lookup::find_class(descriptor.into()), Some(*class));
        for (name, method) in ["first", "second"].into_iter().zip(methods) {
            assert_eq!(
                lookup::class_methods_by_name(*class, name.into()),
                [*method]
            );
            assert_eq!(
                lookup::get_method_info(*method).unwrap().class_descriptor,
                descriptor
            );
        }
    }
    guard.finish().unwrap();
    let guard = ContextGuard::enter(&mut ctx, RunningBundle::at(dir.path().to_owned())).unwrap();
    assert_eq!(
        lookup::get_class_info(classes[2].0).unwrap().descriptor,
        "LC;"
    );
    assert!(lookup::get_class_info(classes[1].0).is_none());
    assert!(lookup::get_method_info(classes[1].1[0]).is_none());
    assert!(guard.finish().is_err());
}
