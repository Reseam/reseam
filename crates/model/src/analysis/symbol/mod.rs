// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Text that is not a class, method or field descriptor.
// Display is written out because BoltFFI's crate scan reads a struct's
// `#[error(...)]` attribute as one of its own markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorError {
    pub text: String,
    pub expected: &'static str,
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a {} descriptor", self.text, self.expected)
    }
}

impl std::error::Error for DescriptorError {}

impl DescriptorError {
    fn new(text: &str, expected: &'static str) -> Self {
        Self {
            text: text.to_owned(),
            expected,
        }
    }
}

/// A class by its type descriptor: `Lcom/example/Foo;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct ClassRef(String);

impl ClassRef {
    pub fn descriptor(&self) -> &str {
        &self.0
    }

    /// The name Java source writes: `com.example.Foo$Inner`.
    pub fn java_name(&self) -> String {
        self.0[1..self.0.len() - 1].replace('/', ".")
    }

    /// The class from its Java name, `com.example.Foo`, or its descriptor.
    pub fn parse_any(text: &str) -> Result<Self, DescriptorError> {
        if text.starts_with('L') && text.ends_with(';') {
            return text.parse();
        }
        let valid = !text.is_empty()
            && text
                .split('.')
                .all(|part| !part.is_empty() && !part.contains(['/', ';', ' ']));
        if !valid {
            return Err(DescriptorError::new(text, "class"));
        }
        Ok(Self(format!("L{};", text.replace('.', "/"))))
    }

    /// The package in Java form, empty for the default package.
    pub fn package(&self) -> String {
        let name = self.java_name();
        name.rsplit_once('.')
            .map(|(package, _)| package.to_owned())
            .unwrap_or_default()
    }

    /// The class's own name, after its package and before any `$`.
    pub fn simple_name(&self) -> &str {
        let body = &self.0[1..self.0.len() - 1];
        let simple = body.rsplit_once('/').map_or(body, |(_, simple)| simple);
        simple.split('$').next().unwrap_or(simple)
    }
}

impl FromStr for ClassRef {
    type Err = DescriptorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let valid = text.len() > 2
            && text.starts_with('L')
            && text.ends_with(';')
            && !text[1..text.len() - 1].contains([';', '.', ' ']);
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(DescriptorError::new(text, "class"))
        }
    }
}

/// A method: `Lcom/example/Foo;->name(ILjava/lang/String;)V`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct MethodRef {
    pub class: ClassRef,
    pub name: String,
    /// Parameter and return types: `(ILjava/lang/String;)V`.
    pub proto: String,
}

impl FromStr for MethodRef {
    type Err = DescriptorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || DescriptorError::new(text, "method");
        let (class, member) = text.split_once("->").ok_or_else(error)?;
        let open = member.find('(').ok_or_else(error)?;
        let (name, proto) = member.split_at(open);
        let (parameters, returns) = proto[1..].split_once(')').ok_or_else(error)?;
        if name.is_empty()
            || type_list(parameters).is_none()
            || type_list(returns).is_none_or(|types| types.len() != 1)
        {
            return Err(error());
        }
        Ok(Self {
            class: class.parse().map_err(|_| error())?,
            name: name.to_owned(),
            proto: proto.to_owned(),
        })
    }
}

impl MethodRef {
    /// The parameter type descriptors, in order.
    pub fn parameters(&self) -> Vec<&str> {
        let close = self
            .proto
            .find(')')
            .expect("a parsed proto closes its parameters");
        type_list(&self.proto[1..close]).expect("a parsed proto holds type descriptors")
    }

    pub fn return_type(&self) -> &str {
        let close = self
            .proto
            .find(')')
            .expect("a parsed proto closes its parameters");
        &self.proto[close + 1..]
    }
}

/// The type descriptors `text` holds back to back, if that is all it holds.
fn type_list(text: &str) -> Option<Vec<&str>> {
    let mut types = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let element = rest.trim_start_matches('[');
        let length = match *element.as_bytes().first()? {
            b'L' => element.find(';')? + 1,
            b'Z' | b'B' | b'S' | b'C' | b'I' | b'J' | b'F' | b'D' | b'V' => 1,
            _ => return None,
        };
        let (type_, tail) = rest.split_at(rest.len() - element.len() + length);
        types.push(type_);
        rest = tail;
    }
    Some(types)
}

/// A field: `Lcom/example/Foo;->name:I`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct FieldRef {
    pub class: ClassRef,
    pub name: String,
    /// The field's type descriptor.
    pub type_: String,
}

impl FromStr for FieldRef {
    type Err = DescriptorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || DescriptorError::new(text, "field");
        let (class, member) = text.split_once("->").ok_or_else(error)?;
        let (name, type_) = member.split_once(':').ok_or_else(error)?;
        if name.is_empty() || type_.is_empty() {
            return Err(error());
        }
        Ok(Self {
            class: class.parse().map_err(|_| error())?,
            name: name.to_owned(),
            type_: type_.to_owned(),
        })
    }
}

/// A class, method or field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub enum Member {
    Class(ClassRef),
    Method(MethodRef),
    Field(FieldRef),
}

impl FromStr for Member {
    type Err = DescriptorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !text.contains("->") {
            return text.parse().map(Self::Class);
        }
        if text.contains('(') {
            text.parse().map(Self::Method)
        } else {
            text.parse().map(Self::Field)
        }
        .map_err(|_| DescriptorError::new(text, "class, method or field"))
    }
}

macro_rules! descriptor_text {
    ($($type:ty),*) => {$(
        impl TryFrom<String> for $type {
            type Error = DescriptorError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                text.parse()
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> Self {
                value.to_string()
            }
        }
    )*};
}
descriptor_text!(ClassRef, MethodRef, FieldRef, Member);

impl fmt::Display for ClassRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for MethodRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}->{}{}", self.class, self.name, self.proto)
    }
}

impl fmt::Display for FieldRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}->{}:{}", self.class, self.name, self.type_)
    }
}

impl fmt::Display for Member {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class(class) => class.fmt(f),
            Self::Method(method) => method.fmt(f),
            Self::Field(field) => field.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests;
