// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::defined::{Extensions, MethodKey};

/// The extension methods the patched app can run, found from what the app's
/// own code and manifest use.
pub(super) struct Reachable<'e> {
    pub(super) extensions: &'e Extensions,
    methods: HashSet<MethodKey>,
    initialized: HashSet<&'e str>,
    instantiated: HashSet<&'e str>,
    pending: Vec<MethodKey>,
}

impl<'e> Reachable<'e> {
    pub(super) fn new(extensions: &'e Extensions) -> Self {
        Self {
            extensions,
            methods: HashSet::new(),
            initialized: HashSet::new(),
            instantiated: HashSet::new(),
            pending: Vec::new(),
        }
    }

    pub(super) fn call(&mut self, key: &MethodKey) {
        if let Some(target) = self.extensions.resolve(key) {
            self.initialize(&target.class);
            self.reach(target);
        }
    }

    fn reach(&mut self, key: MethodKey) {
        if self.methods.insert(key.clone()) {
            self.pending.push(key);
        }
    }

    /// Runs the static initializers Android runs before `class` is first
    /// used: its own and its superclasses'.
    pub(super) fn initialize(&mut self, class: &str) {
        let extensions = self.extensions;
        for (name, _) in extensions.lineage(class) {
            if !self.initialized.insert(name) {
                return;
            }
            let initializer = MethodKey::named(name, "<clinit>", "()V");
            if extensions.methods.contains_key(&initializer) {
                self.reach(initializer);
            }
        }
    }

    /// Instances of `class` exist, so the app or the platform can call any
    /// virtual method they have, including inherited ones.
    fn instantiate(&mut self, class: &str) {
        let extensions = self.extensions;
        for (name, class) in extensions.lineage(class) {
            if !self.instantiated.insert(name) {
                return;
            }
            for key in &class.methods {
                if extensions.methods[key].virtual_ {
                    self.reach(key.clone());
                }
            }
        }
    }

    /// Android creates manifest components itself, through their no-argument
    /// constructor.
    pub(super) fn create_component(&mut self, class: &str) {
        self.initialize(class);
        self.instantiate(class);
        self.call(&MethodKey::named(class, "<init>", "()V"));
    }

    pub(super) fn follow(&mut self) {
        let extensions = self.extensions;
        while let Some(key) = self.pending.pop() {
            let method = &extensions.methods[&key];
            if key.name == "<init>" {
                self.instantiate(&key.class);
            }
            for callee in &method.calls {
                self.call(callee);
            }
            for class in &method.fields_of {
                self.initialize(class);
            }
        }
    }

    pub(super) fn unimplemented(&self) -> Vec<String> {
        let mut missing: Vec<_> = self
            .methods
            .iter()
            .filter(|key| self.extensions.methods[*key].native)
            .collect();
        missing.sort();
        missing.into_iter().map(ToString::to_string).collect()
    }
}
