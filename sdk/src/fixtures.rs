// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! An APK built for tests of analysis, here and in hosts that build on it.

use std::io::Read;
use std::sync::Arc;

use reseam_apk::resources::{
    EntryValue, ResEntry, ResPackage, ResType, ResourceScope, ResourceTable, TypeSpec,
    config_for_qualifiers,
};
use reseam_apk::{ResValue, StringEncoding, StringPool};
use reseam_dex::{
    AccessFlags, CodeItem, DexFile, DexHeader, DexVersion, EncodedField, EncodedMethod,
    Instruction, RegList,
};
use reseam_model::analysis::MethodRef;

/// The id of `string/upgrade`.
const UPGRADE: u32 = 0x7f01_0000;

const OBJECT: &str = "Ljava/lang/Object;";

/// An app whose launcher activity creates an obfuscated entitlement class
/// and asks it whether the user is Pro, directly and through its interface,
/// with an "Upgrade to Pro" string that a layout shows and the activity loads.
pub fn app() -> tempfile::TempPath {
    let mut table = ResourceTable::new(
        pool(&["Upgrade to Pro", "res/layout/paywall.xml"]),
        vec![package()],
    );
    let layout = reseam_apk::axml::compile_xml(
        r#"<TextView xmlns:android="http://schemas.android.com/apk/res/android" android:id="@id/title" android:text="@string/upgrade" />"#,
        Some(&mut ResourceScope::from(&mut table)),
    )
    .expect("fixture layout");
    let manifest = reseam_apk::axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="7" android:versionName="1.2">
    <application android:name=".App">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>"#,
        None,
    )
    .expect("fixture manifest");
    let apk = reseam_fixtures::apk(
        &manifest,
        &[
            ("classes.dex", &dex()),
            ("resources.arsc", &table.serialize().expect("fixture table")),
            ("res/layout/paywall.xml", &layout),
        ],
    );
    let file = tempfile::Builder::new()
        .suffix(".apk")
        .tempfile()
        .expect("temp file");
    std::fs::write(file.path(), apk).expect("write fixture");
    file.into_temp_path()
}

fn pool(values: &[&str]) -> StringPool {
    StringPool::new(
        values.iter().map(ToString::to_string).collect(),
        StringEncoding::Utf8,
    )
}

/// `string/upgrade`, `layout/paywall` and `id/title`, one type each.
fn package() -> ResPackage {
    let mut package = ResPackage::new(
        0x7f,
        "com.example.test",
        pool(&["string", "layout", "id"]),
        pool(&["upgrade", "paywall", "title"]),
    );
    for (type_id, (key, value)) in (1..).zip([
        (0, ResValue::string(0)),
        (1, ResValue::string(1)),
        (2, ResValue::new(ResValue::INT_BOOLEAN, 0)),
    ]) {
        let mut res_type = ResType::new(type_id, config_for_qualifiers("", 64).expect("default"));
        res_type
            .set(
                0,
                Some(ResEntry {
                    flags: 0,
                    key,
                    value: EntryValue::Simple(value),
                }),
            )
            .expect("fixture entry");
        package.add_type_spec(TypeSpec::new(type_id, vec![0]));
        package.add_type(res_type);
    }
    package
}

fn dex() -> Vec<u8> {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V039));
    activity(&mut dex);
    entitlement(&mut dex);
    caller(&mut dex);
    lambdas(&mut dex);
    let (spooled, _) = reseam_dex::write_spooled(&dex, None).expect("write fixture DEX");
    let mut bytes = Vec::new();
    spooled
        .reader()
        .read_to_end(&mut bytes)
        .expect("read fixture DEX");
    bytes
}

/// The launcher: loads the upgrade string's id, creates the repository and
/// asks it whether the user is Pro.
fn activity(dex: &mut DexFile) {
    let class = define(
        dex,
        "Lcom/example/test/MainActivity;",
        AccessFlags::PUBLIC,
        "Landroid/app/Activity;",
    );
    let repository = dex.intern_type("Lb7/r;").expect("type");
    let init = method(dex, "Lb7/r;-><init>()V");
    let is_pro = method(dex, "Lb7/r;->c()Z");
    add(
        dex,
        class,
        "Lcom/example/test/MainActivity;->onCreate(Landroid/os/Bundle;)V",
        AccessFlags::PUBLIC,
        Some((
            3,
            2,
            vec![
                Instruction::Const {
                    dest: 0,
                    value: UPGRADE as i32,
                },
                Instruction::NewInstance {
                    dest: 0,
                    type_: repository,
                },
                Instruction::InvokeDirect {
                    method: init,
                    args: RegList::from([0]),
                },
                Instruction::InvokeVirtual {
                    method: is_pro,
                    args: RegList::from([0]),
                },
                Instruction::ReturnVoid,
            ],
        )),
    );
}

/// `Lb7/q;`, an interface asking whether the user is Pro, and `Lb7/r;`, the
/// obfuscated repository implementing it with field `e`.
fn entitlement(dex: &mut DexFile) {
    let interface = define(
        dex,
        "Lb7/q;",
        AccessFlags::PUBLIC | AccessFlags::INTERFACE | AccessFlags::ABSTRACT,
        OBJECT,
    );
    add(
        dex,
        interface,
        "Lb7/q;->c()Z",
        AccessFlags::PUBLIC | AccessFlags::ABSTRACT,
        None,
    );
    let class = define(
        dex,
        "Lb7/r;",
        AccessFlags::PUBLIC | AccessFlags::FINAL,
        OBJECT,
    );
    let interface_type = dex.intern_type("Lb7/q;").expect("type");
    let pro = dex.intern_field("Lb7/r;", "e", "Z").expect("field");
    let object_init = method(dex, "Ljava/lang/Object;-><init>()V");
    let name = dex.intern_string("EntitlementRepository(");
    let repository = dex.class_mut(class).expect("repository");
    repository.interfaces.push(interface_type);
    repository.add_instance_field(EncodedField {
        field: pro,
        access_flags: AccessFlags::PRIVATE,
    });
    add(
        dex,
        class,
        "Lb7/r;-><init>()V",
        AccessFlags::PUBLIC | AccessFlags::CONSTRUCTOR,
        Some((
            2,
            1,
            vec![
                Instruction::InvokeDirect {
                    method: object_init,
                    args: RegList::from([1]),
                },
                Instruction::Const4 { dest: 0, value: 1 },
                Instruction::IputBoolean {
                    src: 0,
                    obj: 1,
                    field: pro,
                },
                Instruction::ReturnVoid,
            ],
        )),
    );
    add(
        dex,
        class,
        "Lb7/r;->c()Z",
        AccessFlags::PUBLIC,
        Some((
            2,
            1,
            vec![
                Instruction::IgetBoolean {
                    dest: 0,
                    obj: 1,
                    field: pro,
                },
                Instruction::Return { src: 0 },
            ],
        )),
    );
    add(
        dex,
        class,
        "Lb7/r;->toString()Ljava/lang/String;",
        AccessFlags::PUBLIC,
        Some((
            2,
            1,
            vec![
                Instruction::ConstString {
                    dest: 0,
                    string: name,
                },
                Instruction::ReturnObject { src: 0 },
            ],
        )),
    );
}

/// `Lb7/s;`, which asks through the interface.
fn caller(dex: &mut DexFile) {
    let class = define(dex, "Lb7/s;", AccessFlags::PUBLIC, OBJECT);
    let is_pro = method(dex, "Lb7/q;->c()Z");
    add(
        dex,
        class,
        "Lb7/s;->a(Lb7/q;)Z",
        AccessFlags::PUBLIC | AccessFlags::STATIC,
        Some((
            2,
            1,
            vec![
                Instruction::InvokeInterface {
                    method: is_pro,
                    args: RegList::from([1]),
                },
                Instruction::MoveResult { dest: 0 },
                Instruction::Return { src: 0 },
            ],
        )),
    );
}

/// `Lb8/f;`, an interface with `invoke()` that 21 classes implement, as Kotlin's function
/// interfaces are, and `Lb8/u;->run(Lb8/f;)V`, which calls through it.
fn lambdas(dex: &mut DexFile) {
    let interface = define(
        dex,
        "Lb8/f;",
        AccessFlags::PUBLIC | AccessFlags::INTERFACE | AccessFlags::ABSTRACT,
        OBJECT,
    );
    add(
        dex,
        interface,
        "Lb8/f;->invoke()V",
        AccessFlags::PUBLIC | AccessFlags::ABSTRACT,
        None,
    );
    let interface_type = dex.intern_type("Lb8/f;").expect("type");
    for index in 0..21 {
        let descriptor = format!("Lb8/l{index};");
        let class = define(dex, &descriptor, AccessFlags::FINAL, OBJECT);
        dex.class_mut(class)
            .expect("lambda")
            .interfaces
            .push(interface_type);
        add(
            dex,
            class,
            &format!("{descriptor}->invoke()V"),
            AccessFlags::PUBLIC | AccessFlags::FINAL,
            Some((1, 1, vec![Instruction::ReturnVoid])),
        );
    }
    let caller = define(dex, "Lb8/u;", AccessFlags::PUBLIC, OBJECT);
    let invoke = method(dex, "Lb8/f;->invoke()V");
    add(
        dex,
        caller,
        "Lb8/u;->run(Lb8/f;)V",
        AccessFlags::PUBLIC | AccessFlags::STATIC,
        Some((
            1,
            1,
            vec![
                Instruction::InvokeInterface {
                    method: invoke,
                    args: RegList::from([0]),
                },
                Instruction::ReturnVoid,
            ],
        )),
    );
}

fn define(dex: &mut DexFile, descriptor: &str, access: AccessFlags, superclass: &str) -> usize {
    dex.create_class(descriptor, access, Some(superclass))
        .expect("fixture class")
}

fn method(dex: &mut DexFile, descriptor: &str) -> reseam_dex::MethodIdx {
    let method: MethodRef = descriptor.parse().expect("fixture descriptor");
    dex.intern_method(method.class.descriptor(), &method.name, &method.proto)
        .expect("fixture method")
}

/// Adds method `descriptor` to `class` with a body of `(registers, ins, instructions)`.
fn add(
    dex: &mut DexFile,
    class: usize,
    descriptor: &str,
    access_flags: AccessFlags,
    body: Option<(u16, u16, Vec<Instruction>)>,
) {
    let method = method(dex, descriptor);
    let code = body.map(|(registers, ins, instructions)| {
        Arc::new(CodeItem::new(registers, ins, 0, instructions).expect("fixture frame"))
    });
    dex.class_mut(class)
        .expect("fixture class")
        .add_method(EncodedMethod {
            method,
            access_flags,
            code,
        });
}
