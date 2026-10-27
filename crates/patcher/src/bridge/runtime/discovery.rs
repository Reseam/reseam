// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Finds a bundle's patches by reflection over the members its declaration
//! index names.

use std::collections::HashSet;

use jni::objects::{JClass, JObject, JObjectArray, JValue, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::jni_values::{object, string};
use super::jvm::{self, jvm_err};
use crate::bundle::index::{Declaration, MemberKind};
use crate::error::Result;

/// A patch object and the id it is declared under; an object declared under
/// several ids keeps the least.
pub(in crate::bridge) struct Found {
    pub(in crate::bridge) object: jni::objects::Global<JObject<'static>>,
    pub(in crate::bridge) declaration: String,
}

/// The patches `declarations` names in `class`, appended to `found`. Reflection
/// order sets the input order of independent patches. Only indexed members run;
/// reflection also checks their accessibility and arguments.
pub(in crate::bridge) fn read_declarations(
    env: &mut Env<'_>,
    class: &JClass<'_>,
    declarations: &[Declaration],
    patch_class: &JClass<'_>,
    external_class: &JClass<'_>,
    found: &mut Vec<Found>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for kind in [MemberKind::Field, MemberKind::Method] {
        let members = members(env, class, kind)?;
        for index in 0..members.len(env)? {
            jvm::with_frame(env, |env| {
                let member = members.get_element(env, index)?;
                let Some(declaration) = indexed(env, &member, kind, declarations, &mut seen)?
                else {
                    return Ok(());
                };
                let value = member_value(env, &member, kind).map_err(|error| {
                    jvm_err(format!(
                        "patch declaration {} ({}.{}): {error}",
                        declaration.id, declaration.class_name, declaration.member
                    ))
                })?;
                if value.is_null() || !env.is_instance_of(&value, patch_class)? {
                    return Err(jvm_err(format!(
                        "patch declaration {} did not return a ReseamPatch",
                        declaration.id
                    )));
                }
                if !env.is_instance_of(&value, external_class)? {
                    retain(env, found, &value, &declaration.id)?;
                }
                Ok(())
            })?;
        }
    }
    if seen.len() != declarations.len() {
        return Err(jvm_err(
            "indexed patch member is absent from its declaration class",
        ));
    }
    Ok(())
}

fn members<'a>(
    env: &mut Env<'a>,
    class: &JClass<'_>,
    kind: MemberKind,
) -> Result<JObjectArray<'a, JObject<'a>>> {
    let (getter, signature) = match kind {
        MemberKind::Field => ("getFields", "()[Ljava/lang/reflect/Field;"),
        MemberKind::Method => ("getMethods", "()[Ljava/lang/reflect/Method;"),
    };
    let members = object(env, class, getter, signature)?;
    Ok(JObjectArray::<JObject<'_>>::cast_local(env, members)?)
}

/// The declaration `member` is, the first time it is met. Methods with
/// parameters are never declarations.
fn indexed<'d>(
    env: &mut Env<'_>,
    member: &JObject<'_>,
    kind: MemberKind,
    declarations: &'d [Declaration],
    seen: &mut HashSet<usize>,
) -> Result<Option<&'d Declaration>> {
    if kind == MemberKind::Method
        && env
            .call_method(member, jni_str!("getParameterCount"), jni_sig!("()I"), &[])
            .and_then(JValueOwned::i)?
            != 0
    {
        return Ok(None);
    }
    let owner = object(env, member, "getDeclaringClass", "()Ljava/lang/Class;")?;
    let name = string(env, member, "getName")?;
    let owner = string(env, &owner, "getName")?;
    Ok(declarations
        .iter()
        .enumerate()
        .find(|(_, declaration)| {
            declaration.kind == kind && declaration.member == name && declaration.owner == owner
        })
        .filter(|&(index, _)| seen.insert(index))
        .map(|(_, declaration)| declaration))
}

/// The value of a static field, or of a static method called without arguments.
fn member_value<'a>(
    env: &mut Env<'a>,
    member: &JObject<'_>,
    kind: MemberKind,
) -> jni::errors::Result<JObject<'a>> {
    let null = JObject::null();
    match kind {
        MemberKind::Field => env.call_method(
            member,
            jni_str!("get"),
            jni_sig!("(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[JValue::Object(&null)],
        ),
        MemberKind::Method => env.call_method(
            member,
            jni_str!("invoke"),
            jni_sig!("(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;"),
            &[JValue::Object(&null), JValue::Object(&null)],
        ),
    }
    .and_then(JValueOwned::l)
}

fn retain(env: &Env<'_>, found: &mut Vec<Found>, object: &JObject<'_>, id: &str) -> Result<()> {
    for seen in found.iter_mut() {
        if env
            .is_same_object(seen.object.as_ref(), object)
            .map_err(|error| jvm_err(format!("compare patch {id}: {error}")))?
        {
            if id < seen.declaration.as_str() {
                id.clone_into(&mut seen.declaration);
            }
            return Ok(());
        }
    }
    found.push(Found {
        object: env
            .new_global_ref(object)
            .map_err(|error| jvm_err(format!("retain patch {id}: {error}")))?,
        declaration: id.to_owned(),
    });
    Ok(())
}
