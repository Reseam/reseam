// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::fmt;

use reseam_dex::{AccessFlags, DexFile, MethodIdx, MethodKind, MethodView, RefQuery};

use crate::error::Result;

/// A method as code names it.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct MethodKey {
    pub(super) class: String,
    pub(super) name: String,
    pub(super) proto: String,
}

impl MethodKey {
    pub(super) fn of(dex: &DexFile, method: MethodIdx) -> Self {
        let id = dex.method_id(method);
        Self {
            class: dex.type_descriptor(id.class).into_owned(),
            name: dex.strings().get(id.name).into_owned(),
            proto: dex.proto_descriptor(&dex.proto(id.proto)),
        }
    }

    pub(super) fn named(class: &str, name: &str, proto: &str) -> Self {
        Self {
            class: class.to_owned(),
            name: name.to_owned(),
            proto: proto.to_owned(),
        }
    }
}

impl fmt::Display for MethodKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}->{}{}", self.class, self.name, self.proto)
    }
}

/// A method a linked extension defines, and what its code uses.
pub(super) struct Method {
    pub(super) native: bool,
    /// Callable through any instance, whoever names the method.
    pub(super) virtual_: bool,
    pub(super) calls: Vec<MethodKey>,
    /// Classes whose fields the code uses, which initializes them.
    pub(super) fields_of: Vec<String>,
}

pub(super) struct Class {
    pub(super) superclass: Option<String>,
    pub(super) methods: Vec<MethodKey>,
}

/// The classes and methods every linked extension defines.
#[derive(Default)]
pub(super) struct Extensions {
    pub(super) classes: HashMap<String, Class>,
    pub(super) methods: HashMap<MethodKey, Method>,
}

impl Extensions {
    pub(super) fn add(&mut self, dex: &DexFile) -> Result<()> {
        for header in dex.classes().headers() {
            self.classes.insert(
                dex.type_descriptor(header.class_type).into_owned(),
                Class {
                    superclass: header
                        .superclass
                        .map(|class| dex.type_descriptor(class).into_owned()),
                    methods: Vec::new(),
                },
            );
        }
        for (key, method) in
            dex.scan_methods_collect(&RefQuery::default(), |view| read(dex, view).map(Some))?
        {
            self.classes
                .get_mut(&key.class)
                .expect("a DEX defines the classes of the methods it defines")
                .methods
                .push(key.clone());
            self.methods.insert(key, method);
        }
        Ok(())
    }

    /// The method a call to `key` runs when extensions define it: the one
    /// `key`'s class declares, or inherits from a superclass they define.
    pub(super) fn resolve(&self, key: &MethodKey) -> Option<MethodKey> {
        let mut class = self.classes.get_key_value(&key.class)?;
        loop {
            let candidate = MethodKey::named(class.0, &key.name, &key.proto);
            if self.methods.contains_key(&candidate) {
                return Some(candidate);
            }
            class = self.classes.get_key_value(class.1.superclass.as_deref()?)?;
        }
    }

    /// `class` and each superclass extensions define, subclass first.
    pub(super) fn lineage<'e>(
        &'e self,
        class: &str,
    ) -> impl Iterator<Item = (&'e String, &'e Class)> {
        std::iter::successors(self.classes.get_key_value(class), |(_, class)| {
            self.classes.get_key_value(class.superclass.as_deref()?)
        })
    }
}

fn read(dex: &DexFile, view: &MethodView<'_>) -> reseam_dex::Result<(MethodKey, Method)> {
    let mut method = Method {
        native: view.access_flags.contains(AccessFlags::NATIVE),
        virtual_: view.kind == MethodKind::Virtual,
        calls: Vec::new(),
        fields_of: Vec::new(),
    };
    view.for_each_instruction(|instruction| {
        if let Some(callee) = instruction.method_ref() {
            method.calls.push(MethodKey::of(dex, callee));
        }
        if let Some(field) = instruction.field_ref() {
            let class = dex.field_id(field).class;
            method
                .fields_of
                .push(dex.type_descriptor(class).into_owned());
        }
        true
    })?;
    Ok((MethodKey::of(dex, view.method), method))
}
