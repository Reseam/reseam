// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::reseam_dex::{
    DexFile, EncodedField, EncodedMethod, MemberCounts, MethodIdx, MethodSummary,
    MultiDexContainer, summarize_resident,
};

use reseam_apk::reseam_dex::MethodKind;

use crate::error::Result;

use super::{CachedMethod, CachedSkeleton, ClassLocation, MethodLocation, PatchContext};

impl PatchContext<'_> {
    pub fn dex(&self) -> &MultiDexContainer {
        self.apk.dex()
    }

    pub fn dex_file(&self, index: usize) -> Option<&DexFile> {
        self.apk.dex().dex(index)
    }

    /// One DEX for whole-file operations such as interning, without
    /// resolving any class data.
    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.method = None;
        self.skeleton = None;
        self.apk.dex_mut(index)
    }

    /// Reads one method for inspection without materializing its class. The
    /// decode is cached until the next mutable DEX access, so read-only FFIs
    /// that walk a method instruction by instruction decode it once.
    pub fn read_method(
        &mut self,
        location: MethodLocation,
    ) -> Result<Option<(&DexFile, &EncodedMethod)>> {
        if let Some(dex) = self.apk.dex().dex(location.dex_idx)
            && let Some(class) = dex.resident_class(location.class_idx)
        {
            let method = class
                .class_data
                .as_ref()
                .and_then(|data| match location.kind {
                    MethodKind::Direct => data.direct_methods.get(location.method_idx),
                    MethodKind::Virtual => data.virtual_methods.get(location.method_idx),
                });
            return Ok(method.map(|method| (dex, method)));
        }
        if self
            .method
            .as_ref()
            .is_none_or(|cached| cached.location != location)
        {
            let Some(dex) = self.dex_file(location.dex_idx) else {
                return Ok(None);
            };
            let Some(method) =
                dex.decode_method_at(location.class_idx, location.method_idx, location.kind)?
            else {
                return Ok(None);
            };
            self.method = Some(CachedMethod { location, method });
        }
        Ok(Some((
            self.apk
                .dex()
                .dex(location.dex_idx)
                .expect("cached method has a DEX"),
            &self
                .method
                .as_ref()
                .expect("method was decoded above")
                .method,
        )))
    }

    /// Reads identity and frame shape without decoding code. The deferred class
    /// skeleton is cached until any mutable DEX access invalidates it.
    pub fn read_method_summary(
        &mut self,
        location: MethodLocation,
    ) -> Result<Option<MethodSummary>> {
        let Some(dex) = self.apk.dex().dex(location.dex_idx) else {
            return Ok(None);
        };
        if let Some(data) = dex
            .resident_class(location.class_idx)
            .and_then(|c| c.class_data.as_deref())
        {
            let list = if location.kind == MethodKind::Virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return Ok(list.get(location.method_idx).map(summarize_resident));
        }
        let class = ClassLocation {
            dex_idx: location.dex_idx,
            class_idx: location.class_idx,
        };
        if self
            .skeleton
            .as_ref()
            .is_none_or(|cached| cached.location != class)
        {
            let Some(skeleton) = dex.class_skeleton(location.class_idx)? else {
                return Ok(None);
            };
            self.skeleton = Some(CachedSkeleton {
                location: class,
                skeleton,
            });
        }
        let Some(header) = self
            .skeleton
            .as_ref()
            .expect("class skeleton was read above")
            .skeleton
            .method(location.method_idx, location.kind)
        else {
            return Ok(None);
        };
        Ok(Some(dex.summarize_method(header)?))
    }

    pub fn read_class_counts(&self, location: ClassLocation) -> Result<Option<MemberCounts>> {
        let Some(dex) = self.dex_file(location.dex_idx) else {
            return Ok(None);
        };
        Ok(dex.class_member_counts(location.class_idx)?)
    }

    /// Reads fields and their DEX without decoding method bodies.
    pub fn read_class_fields(
        &self,
        location: ClassLocation,
    ) -> Result<Option<(&DexFile, ClassFields)>> {
        let Some(dex) = self.dex_file(location.dex_idx) else {
            return Ok(None);
        };
        Ok(dex
            .decode_class_fields(location.class_idx)?
            .map(|(statics, instances)| (dex, ClassFields { statics, instances })))
    }

    /// Materializes one class and invalidates inspection caches before mutation.
    pub fn class_dex_mut(
        &mut self,
        dex_idx: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        self.method = None;
        self.skeleton = None;
        Ok(self.apk.resolve_dex_class_mut(dex_idx, class_idx)?)
    }

    pub fn find_class(&self, descriptor: &str) -> Option<ClassLocation> {
        self.dex().iter().enumerate().find_map(|(dex_idx, dex)| {
            dex.find_class_index(descriptor)
                .map(|class_idx| ClassLocation { dex_idx, class_idx })
        })
    }

    /// Superclasses nearest first. A base class often sits in another DEX of a
    /// multi-dex app, so the walk resolves each superclass by descriptor across
    /// the whole app and stops where the app stops defining them.
    pub fn superclass_chain(&self, class: ClassLocation) -> Vec<ClassLocation> {
        let mut chain: Vec<ClassLocation> = Vec::new();
        let mut current = class;
        while let Some(superclass) = self.dex_file(current.dex_idx).and_then(|dex| {
            let header = dex.class_header(current.class_idx);
            header
                .superclass
                .map(|ty| dex.type_descriptor(ty).into_owned())
        }) {
            let Some(next) = self.find_class(&superclass) else {
                break;
            };
            if next == class || chain.contains(&next) {
                break;
            }
            chain.push(next);
            current = next;
        }
        chain
    }

    /// Locates a method by class and name without materializing the class:
    /// resident classes are searched through their IR, others through the
    /// raw member list.
    pub fn find_method(
        &self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Result<Option<MethodLocation>> {
        self.scan_first("method", |dex_idx, dex| {
            let (Some(class_idx), Some(name)) = (
                dex.find_class_index(class_descriptor),
                dex.find_string_idx(method_name),
            ) else {
                return Ok(None);
            };
            let named = |method: MethodIdx| dex.method_id(method).name == name;
            let slot = match dex.resident_class(class_idx) {
                Some(class) => class.class_data.as_ref().and_then(|data| {
                    find_slot(
                        data.direct_methods.iter().map(|m| m.method),
                        data.virtual_methods.iter().map(|m| m.method),
                        named,
                    )
                }),
                None => dex.class_skeleton(class_idx)?.and_then(|skeleton| {
                    find_slot(
                        skeleton.direct_methods.iter().map(|m| m.method),
                        skeleton.virtual_methods.iter().map(|m| m.method),
                        named,
                    )
                }),
            };
            Ok(slot.map(|(method_idx, kind)| MethodLocation {
                dex_idx,
                class_idx,
                method_idx,
                kind,
            }))
        })
    }
}

fn find_slot(
    mut direct: impl Iterator<Item = MethodIdx>,
    mut virtual_: impl Iterator<Item = MethodIdx>,
    named: impl Fn(MethodIdx) -> bool,
) -> Option<(usize, MethodKind)> {
    direct
        .position(&named)
        .map(|pos| (pos, MethodKind::Direct))
        .or_else(|| {
            virtual_
                .position(named)
                .map(|pos| (pos, MethodKind::Virtual))
        })
}

pub struct ClassFields {
    pub statics: Vec<EncodedField>,
    pub instances: Vec<EncodedField>,
}
