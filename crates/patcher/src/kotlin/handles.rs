// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-run state the exported functions reach through thread-locals: the
//! active patch context and the integer handles patches hold to methods
//! and classes.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use reseam_apk::reseam_dex::{CodeItem, DexFile, EncodedMethod};
use rustc_hash::FxHashMap;

use super::xml;
use crate::context::{ClassLocation, MethodLocation, PatchContext};

thread_local! {
    static CTX_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
    static BUNDLE_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Makes `ctx` the active context for the exported functions until dropped.
pub(super) struct ContextGuard;

impl ContextGuard {
    pub fn enter(ctx: &mut PatchContext<'_>, bundle_dir: PathBuf) -> Self {
        CTX_PTR.with(|cell| {
            assert!(cell.get().is_null(), "nested patch context");
            cell.set(ctx as *mut PatchContext as *mut ());
        });
        BUNDLE_DIR.with(|dir| *dir.borrow_mut() = Some(bundle_dir));
        Self
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        CTX_PTR.with(|cell| cell.set(std::ptr::null_mut()));
        BUNDLE_DIR.with(|dir| *dir.borrow_mut() = None);
        HANDLES.with(|h| *h.borrow_mut() = HandleTable::default());
        xml::reset();
    }
}

pub(super) fn context_is_active() -> bool {
    CTX_PTR.with(|cell| !cell.get().is_null())
}

pub(super) fn with_ctx<R>(f: impl FnOnce(&mut PatchContext<'_>) -> R) -> R {
    CTX_PTR.with(|cell| {
        let ptr = cell.get();
        assert!(!ptr.is_null(), "patch context is not active");
        // SAFETY: the pointer was set from a live `&mut PatchContext` by
        // `ContextGuard::enter` on this thread and is cleared before that
        // borrow ends; exported functions run on the same thread, one at a time.
        f(unsafe { &mut *(ptr as *mut PatchContext<'_>) })
    })
}

/// A path inside the bundle's extracted directory.
pub(super) fn bundle_path(relative: &str) -> PathBuf {
    BUNDLE_DIR.with(|dir| match dir.borrow().as_ref() {
        Some(dir) => dir.join(relative),
        None => PathBuf::from(relative),
    })
}

/// Handles handed to patches. Each location packs into one word, and keys
/// allocated in increasing order (a full-app enumeration) form a sorted
/// prefix found by binary search, so only out-of-order allocations pay for a
/// hash entry.
#[derive(Default)]
pub(super) struct HandleTable {
    methods: Handles,
    classes: Handles,
}

#[derive(Default)]
struct Handles {
    keys: Vec<u64>,
    sorted_len: usize,
    index: FxHashMap<u64, u32>,
}

impl Handles {
    fn alloc(&mut self, key: u64) -> u32 {
        if let Ok(i) = self.keys[..self.sorted_len].binary_search(&key) {
            return i as u32;
        }
        if let Some(&h) = self.index.get(&key) {
            return h;
        }
        let h = self.keys.len() as u32;
        if self.sorted_len == self.keys.len() && self.keys.last().is_none_or(|&last| last < key) {
            self.sorted_len += 1;
        } else {
            self.index.insert(key, h);
        }
        self.keys.push(key);
        h
    }

    fn get(&self, handle: u32) -> Option<u64> {
        self.keys.get(handle as usize).copied()
    }
}

impl HandleTable {
    pub fn alloc_method(&mut self, m: MethodLocation) -> u32 {
        debug_assert!(m.dex_idx < 256 && m.class_idx < 1 << 32 && m.method_idx < 1 << 16);
        let key = (m.dex_idx as u64) << 56
            | (m.class_idx as u64) << 24
            | (m.is_virtual as u64) << 16
            | m.method_idx as u64;
        self.methods.alloc(key)
    }

    pub fn get_method(&self, handle: u32) -> Option<MethodLocation> {
        let key = self.methods.get(handle)?;
        Some(MethodLocation {
            dex_idx: (key >> 56) as usize,
            class_idx: (key >> 24) as u32 as usize,
            method_idx: key as u16 as usize,
            is_virtual: (key >> 16) & 1 == 1,
        })
    }

    pub fn alloc_class(&mut self, c: ClassLocation) -> u32 {
        debug_assert!(c.dex_idx < 256 && c.class_idx < 1 << 32);
        self.classes
            .alloc((c.dex_idx as u64) << 56 | c.class_idx as u64)
    }

    pub fn get_class(&self, handle: u32) -> Option<ClassLocation> {
        let key = self.classes.get(handle)?;
        Some(ClassLocation {
            dex_idx: (key >> 56) as usize,
            class_idx: key as u32 as usize,
        })
    }
}

pub(super) fn alloc_method(location: MethodLocation) -> u32 {
    HANDLES.with(|h| h.borrow_mut().alloc_method(location))
}

pub(super) fn alloc_methods(locations: impl IntoIterator<Item = MethodLocation>) -> Vec<u32> {
    HANDLES.with(|h| {
        let mut h = h.borrow_mut();
        locations.into_iter().map(|l| h.alloc_method(l)).collect()
    })
}

pub(super) fn method_location(handle: u32) -> Option<MethodLocation> {
    HANDLES.with(|h| h.borrow().get_method(handle))
}

pub(super) fn alloc_class(location: ClassLocation) -> u32 {
    HANDLES.with(|h| h.borrow_mut().alloc_class(location))
}

pub(super) fn class_location(handle: u32) -> Option<ClassLocation> {
    HANDLES.with(|h| h.borrow().get_class(handle))
}

/// Read access to the method behind `handle`, decoded once per method.
pub(super) fn with_method<R>(
    handle: u32,
    f: impl FnOnce(&DexFile, &EncodedMethod) -> Option<R>,
) -> Option<R> {
    let location = method_location(handle)?;
    with_ctx(|ctx| {
        let (dex, method) = ctx.read_method(location)?;
        f(dex, method)
    })
}

pub(super) fn with_code<R>(
    handle: u32,
    f: impl FnOnce(&DexFile, &CodeItem) -> Option<R>,
) -> Option<R> {
    with_method(handle, |dex, method| f(dex, method.code.as_ref()?))
}

/// Mutable access to the DEX holding the method behind `handle`, with the
/// method's class materialized; reach the method with [`method_mut`].
pub(super) fn with_method_mut<R>(
    handle: u32,
    f: impl FnOnce(&mut DexFile, MethodLocation) -> Option<R>,
) -> Option<R> {
    let location = method_location(handle)?;
    with_ctx(|ctx| {
        f(
            ctx.class_dex_mut(location.dex_idx, location.class_idx)?,
            location,
        )
    })
}

/// Mutable access to the DEX holding the class behind `handle`, materialized.
pub(super) fn with_class_mut<R>(
    handle: u32,
    f: impl FnOnce(&mut DexFile, ClassLocation) -> Option<R>,
) -> Option<R> {
    let location = class_location(handle)?;
    with_ctx(|ctx| {
        f(
            ctx.class_dex_mut(location.dex_idx, location.class_idx)?,
            location,
        )
    })
}

pub(super) fn method_ref(dex: &DexFile, m: MethodLocation) -> Option<&EncodedMethod> {
    let data = dex.resident_class(m.class_idx)?.class_data.as_ref()?;
    let list = if m.is_virtual {
        &data.virtual_methods
    } else {
        &data.direct_methods
    };
    list.get(m.method_idx)
}

pub(super) fn method_mut(dex: &mut DexFile, m: MethodLocation) -> Option<&mut EncodedMethod> {
    let data = dex.class_mut(m.class_idx).ok()?.class_data.as_mut()?;
    let list = if m.is_virtual {
        &mut data.virtual_methods
    } else {
        &mut data.direct_methods
    };
    list.get_mut(m.method_idx)
}

pub(super) fn code_mut(dex: &mut DexFile, m: MethodLocation) -> Option<&mut CodeItem> {
    method_mut(dex, m)?.code.as_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "patch context is not active")]
    fn with_ctx_requires_active_context() {
        with_ctx(|_| ());
    }

    #[test]
    fn handles_dedup_in_and_out_of_order() {
        let m = |dex_idx, class_idx, method_idx, is_virtual| MethodLocation {
            dex_idx,
            class_idx,
            method_idx,
            is_virtual,
        };
        let mut table = HandleTable::default();
        let a = table.alloc_method(m(0, 5, 0, false));
        let b = table.alloc_method(m(0, 5, 1, false));
        let c = table.alloc_method(m(1, 0, 0, true));
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(table.methods.sorted_len, 3);
        assert!(table.methods.index.is_empty());

        let d = table.alloc_method(m(0, 2, 7, true));
        assert_eq!(d, 3);
        assert_eq!(table.methods.sorted_len, 3);
        assert_eq!(table.alloc_method(m(0, 2, 7, true)), d);
        assert_eq!(table.alloc_method(m(0, 5, 1, false)), b);
        assert_eq!(table.get_method(d), Some(m(0, 2, 7, true)));
        let big = table.alloc_method(m(3, 70_000, 65_000, false));
        assert_eq!(table.get_method(big), Some(m(3, 70_000, 65_000, false)));

        let k = table.alloc_class(ClassLocation {
            dex_idx: 2,
            class_idx: 9,
        });
        assert_eq!(
            table.alloc_class(ClassLocation {
                dex_idx: 2,
                class_idx: 9
            }),
            k
        );
        assert_eq!(
            table.get_class(k),
            Some(ClassLocation {
                dex_idx: 2,
                class_idx: 9
            })
        );
    }
}
