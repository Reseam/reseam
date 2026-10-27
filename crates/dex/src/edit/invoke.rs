// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::instruction::operands::RegisterKind;
use crate::model::descriptor::{is_type_descriptor, parse_method_descriptor};

/// How an invoke or filled-new-array names its argument words: as a list of
/// at most five registers below v16, or as a range of consecutive registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokeForm {
    Compact,
    Range,
}

/// Whether a method takes its receiver as its first argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    Present,
    Absent,
}

/// A copy of one argument into the span a range form reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgumentMove {
    pub dest: u16,
    pub src: u16,
    pub kind: RegisterKind,
}

/// Where a lowered invocation reads its arguments: the registers it was
/// given, as a list, or a range of `count` words from `first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokeEncoding {
    Compact,
    Range { first: u16, count: u8 },
}

/// The argument words of an invocation, each argument's words in order.
/// Construction checks them against the kinds of the arguments they carry.
pub struct Invocation<'a> {
    form: InvokeForm,
    registers: &'a [u16],
    kinds: &'a [RegisterKind],
}

impl<'a> Invocation<'a> {
    pub fn new(form: InvokeForm, registers: &'a [u16], kinds: &'a [RegisterKind]) -> Result<Self> {
        u8::try_from(registers.len())
            .map_err(|_| invalid("invoke", "invocation exceeds 255 register words"))?;
        if kinds
            .iter()
            .map(|kind| usize::from(kind.words()))
            .sum::<usize>()
            != registers.len()
        {
            return Err(invalid(
                "invoke",
                "register word count does not match the arguments",
            ));
        }
        let mut word = 0;
        for kind in kinds {
            if *kind == RegisterKind::Wide
                && registers[word].checked_add(1) != Some(registers[word + 1])
            {
                return Err(invalid(
                    "invoke",
                    "wide argument must occupy consecutive registers",
                ));
            }
            word += usize::from(kind.words());
        }
        Ok(Self {
            form,
            registers,
            kinds,
        })
    }

    /// How many consecutive registers the arguments must be copied to first;
    /// zero when an encoding reaches them where they are.
    pub fn scratch_words(&self) -> u16 {
        if self.compact() || consecutive(self.registers) {
            0
        } else {
            self.registers.len() as u16
        }
    }

    /// The copies the invocation needs, then the encoding it takes: the
    /// compact form when it was written so and still fits, else the range
    /// form. `scratch` is the first of [`Self::scratch_words`] consecutive
    /// registers no argument occupies.
    pub fn lower(&self, scratch: Option<u16>) -> Result<(Vec<ArgumentMove>, InvokeEncoding)> {
        if self.compact() {
            return Ok((Vec::new(), InvokeEncoding::Compact));
        }
        let count = self.registers.len() as u8;
        if self.scratch_words() == 0 {
            let first = self.registers.first().copied().unwrap_or(0);
            return Ok((Vec::new(), InvokeEncoding::Range { first, count }));
        }
        let first = scratch.ok_or_else(|| invalid("invoke", "arguments need a scratch span"))?;
        let span = u32::from(first)..u32::from(first) + u32::from(count);
        if span.end > 1 << 16
            || self
                .registers
                .iter()
                .any(|&register| span.contains(&u32::from(register)))
        {
            return Err(invalid(
                "invoke",
                "scratch span must fit the frame and exclude the argument registers",
            ));
        }
        let mut moves = Vec::with_capacity(self.kinds.len());
        let mut word = 0;
        for &kind in self.kinds {
            moves.push(ArgumentMove {
                dest: first + word,
                src: self.registers[usize::from(word)],
                kind,
            });
            word += kind.words();
        }
        Ok((moves, InvokeEncoding::Range { first, count }))
    }

    fn compact(&self) -> bool {
        self.form == InvokeForm::Compact
            && self.registers.len() <= 5
            && self.registers.iter().all(|&register| register <= 15)
    }
}

fn consecutive(registers: &[u16]) -> bool {
    registers
        .windows(2)
        .all(|pair| pair[0].checked_add(1) == Some(pair[1]))
}

/// The kinds of the arguments a method with prototype `proto` takes.
pub fn argument_kinds(proto: &str, receiver: Receiver) -> Result<Vec<RegisterKind>> {
    let (parameters, _) = parse_method_descriptor(proto)
        .ok_or_else(|| invalid("invoke", format!("invalid prototype {proto}")))?;
    Ok((receiver == Receiver::Present)
        .then_some(RegisterKind::Object)
        .into_iter()
        .chain(parameters.into_iter().map(RegisterKind::of))
        .collect())
}

/// The kinds of the `count` elements a filled-new-array of type
/// `descriptor` takes, which cannot be wide.
pub fn array_kinds(descriptor: &str, count: usize) -> Result<Vec<RegisterKind>> {
    let element = descriptor
        .strip_prefix('[')
        .filter(|_| is_type_descriptor(descriptor))
        .ok_or_else(|| invalid("invoke", "filled-new-array requires an array descriptor"))?;
    match RegisterKind::of(element) {
        RegisterKind::Wide => Err(invalid(
            "invoke",
            "filled-new-array cannot contain wide primitives",
        )),
        kind => Ok(vec![kind; count]),
    }
}
