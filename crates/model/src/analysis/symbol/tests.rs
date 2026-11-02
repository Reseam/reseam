// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{ClassRef, FieldRef, Member, MethodRef};

#[test]
fn descriptors_parse_and_print_unchanged() {
    for text in [
        "Lb7/r;",
        "Lb7/r;->c()Z",
        "Lcom/example/Foo$Inner;-><init>(Landroid/content/Context;I)V",
        "Lb7/r;->e:Z",
        "Lb7/r;->items:[Ljava/lang/String;",
    ] {
        assert_eq!(text.parse::<Member>().expect(text).to_string(), text);
    }
    for bad in [
        "b7.r",
        "Lb7/r",
        "Lb7/r;->",
        "Lb7/r;->()V",
        "Lb7/r;->:I",
        "Lb7/r;->c(Q)V",
        "Lb7/r;->c()",
        "Lb7/r;->c()ZZ",
        "Lb7/r;->c([)V",
    ] {
        assert!(bad.parse::<Member>().is_err(), "{bad}");
    }
}

#[test]
fn classes_read_from_java_names() {
    let class = ClassRef::parse_any("com.example.Foo$Inner").expect("java name");
    assert_eq!(class.descriptor(), "Lcom/example/Foo$Inner;");
    assert_eq!(class.package(), "com.example");
    assert_eq!(class.simple_name(), "Foo");
    assert!(ClassRef::parse_any("com..Foo").is_err());
    let method: MethodRef = "Lb7/r;->c()Z".parse().expect("method");
    let field: FieldRef = "Lb7/r;->e:Z".parse().expect("field");
    assert_eq!((method.name.as_str(), field.type_.as_str()), ("c", "Z"));
}

#[test]
fn methods_split_their_proto_into_types() {
    let method: MethodRef = "La;->m(I[[Ljava/lang/String;JLb;)[Z"
        .parse()
        .expect("method");
    assert_eq!(
        method.parameters(),
        ["I", "[[Ljava/lang/String;", "J", "Lb;"]
    );
    assert_eq!(method.return_type(), "[Z");
    let method: MethodRef = "La;-><init>()V".parse().expect("constructor");
    assert!(method.parameters().is_empty());
}

#[cfg(feature = "schema")]
mod schema {
    use super::super::{Member, MethodRef};

    #[test]
    fn descriptors_are_strings_in_json_schema() {
        for schema in [
            schemars::schema_for!(MethodRef),
            schemars::schema_for!(Member),
        ] {
            assert_eq!(schema.get("type"), Some(&serde_json::json!("string")));
        }
    }
}
