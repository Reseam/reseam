// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Operands of each Dalvik instruction format, in smali order.

use super::{Delta, Reference, Unsigned};
use crate::error::Result;
use crate::instruction::{Instruction, RegList};
use crate::smali::body::Body;
use crate::smali::labels::LabelKind;
use crate::smali::names::long;

pub(super) use self::{
    f10t as f20t, f10t as f30t, f10x as nop, f11n as f21s, f11n as f21h, f11n as f31i,
    f11n as f51l, f12x as f22x, f12x as f32x, f21c as f31c, f22b as f22s,
};

pub(super) fn f10x(_: &Body<'_>, _: usize, _: &Instruction) -> String {
    String::new()
}

pub(super) fn f10t(body: &Body<'_>, at: usize, _: &Instruction, offset: &impl Delta) -> String {
    body.label(at, offset.delta(), LabelKind::Goto)
}

pub(super) fn f11x(body: &Body<'_>, _: usize, _: &Instruction, a: &impl Unsigned) -> String {
    body.register(a.get())
}

pub(super) fn f12x(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    a: &impl Unsigned,
    b: &impl Unsigned,
) -> String {
    format!("{}, {}", body.register(a.get()), body.register(b.get()))
}

pub(super) fn f23x(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    a: &impl Unsigned,
    b: &impl Unsigned,
    c: &impl Unsigned,
) -> String {
    format!(
        "{}, {}, {}",
        body.register(a.get()),
        body.register(b.get()),
        body.register(c.get())
    )
}

pub(super) fn f11n<T>(
    body: &Body<'_>,
    _: usize,
    insn: &Instruction,
    a: &impl Unsigned,
    _: &T,
) -> String {
    format!(
        "{}, {}",
        body.register(a.get()),
        long(insn.literal().expect("literal formats carry a literal"))
    )
}

pub(super) fn f22b<T>(
    body: &Body<'_>,
    _: usize,
    insn: &Instruction,
    a: &impl Unsigned,
    b: &impl Unsigned,
    _: &T,
) -> String {
    format!(
        "{}, {}, {}",
        body.register(a.get()),
        body.register(b.get()),
        long(insn.literal().expect("literal formats carry a literal"))
    )
}

pub(super) fn f21t(
    body: &Body<'_>,
    at: usize,
    _: &Instruction,
    a: &impl Unsigned,
    offset: &impl Delta,
) -> String {
    format!(
        "{}, {}",
        body.register(a.get()),
        body.label(at, offset.delta(), LabelKind::Cond)
    )
}

pub(super) fn f22t(
    body: &Body<'_>,
    at: usize,
    _: &Instruction,
    a: &impl Unsigned,
    b: &impl Unsigned,
    offset: &impl Delta,
) -> String {
    format!(
        "{}, {}, {}",
        body.register(a.get()),
        body.register(b.get()),
        body.label(at, offset.delta(), LabelKind::Cond)
    )
}

pub(super) fn f31t(
    body: &Body<'_>,
    at: usize,
    insn: &Instruction,
    a: &impl Unsigned,
    offset: &impl Delta,
) -> String {
    let (kind, _) = LabelKind::payload(insn);
    format!(
        "{}, {}",
        body.register(a.get()),
        body.label(at, offset.delta(), kind)
    )
}

pub(super) fn f21c(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    a: &impl Unsigned,
    reference: &impl Reference,
) -> Result<String> {
    Ok(format!(
        "{}, {}",
        body.register(a.get()),
        reference.name(body.names)?
    ))
}

pub(super) fn f22c(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    a: &impl Unsigned,
    b: &impl Unsigned,
    reference: &impl Reference,
) -> Result<String> {
    Ok(format!(
        "{}, {}, {}",
        body.register(a.get()),
        body.register(b.get()),
        reference.name(body.names)?
    ))
}

pub(super) fn f35c(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    reference: &impl Reference,
    args: &RegList,
) -> Result<String> {
    Ok(format!(
        "{}, {}",
        body.list(args),
        reference.name(body.names)?
    ))
}

pub(super) fn f3rc(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    reference: &impl Reference,
    first: &impl Unsigned,
    count: &impl Unsigned,
) -> Result<String> {
    Ok(format!(
        "{}, {}",
        body.range(first.get(), count.get()),
        reference.name(body.names)?
    ))
}

pub(super) fn f45cc(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    method: &impl Reference,
    proto: &impl Reference,
    args: &RegList,
) -> Result<String> {
    Ok(format!(
        "{}, {}, {}",
        body.list(args),
        method.name(body.names)?,
        proto.name(body.names)?
    ))
}

pub(super) fn f4rcc(
    body: &Body<'_>,
    _: usize,
    _: &Instruction,
    method: &impl Reference,
    proto: &impl Reference,
    first: &impl Unsigned,
    count: &impl Unsigned,
) -> Result<String> {
    Ok(format!(
        "{}, {}, {}",
        body.range(first.get(), count.get()),
        method.name(body.names)?,
        proto.name(body.names)?
    ))
}
