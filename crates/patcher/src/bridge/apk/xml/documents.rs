// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::apk::components::with_component;
use crate::bridge::handles::OwnedHandles;
use crate::bridge::run_state::checked;
use crate::context::PatchContext;
use boltffi::export;
use reseam_apk::axml::AxmlDocument;
use reseam_apk::{Compression, StringEncoding};
use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

use super::nodes::Nodes;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::bridge) enum Source {
    File { component: usize, path: String },
    Manifest { component: usize },
    Memory,
}

struct OpenDoc {
    source: Source,
    owned: Option<AxmlDocument>,
    borrowers: u32,
    dirty: bool,
    nodes: Nodes,
}

thread_local! { static DOCS: RefCell<OwnedHandles<OpenDoc>> = RefCell::default(); }

pub(in crate::bridge) fn reset() {
    DOCS.with(|docs| *docs.borrow_mut() = OwnedHandles::default());
}

fn not_open(handle: u32) -> String {
    format!("XML document {handle} is not open")
}

pub(in crate::bridge) fn finish() -> Result<(), String> {
    DOCS.with(|docs| {
        match docs
            .borrow()
            .iter()
            .find(|(_, open)| open.dirty && open.source != Source::Memory)
        {
            Some((handle, _)) => Err(format!(
                "edited XML document {handle} has unclosed borrowers"
            )),
            None => Ok(()),
        }
    })
}

pub(in crate::bridge) fn open(
    source: Source,
    document: Option<AxmlDocument>,
) -> Result<u32, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        if source != Source::Memory
            && let Some((handle, existing)) =
                docs.iter_mut().find(|(_, open)| open.source == source)
        {
            existing.borrowers = existing
                .borrowers
                .checked_add(1)
                .ok_or("too many XML borrowers")?;
            return Ok(handle);
        }
        docs.insert(OpenDoc {
            source,
            owned: document,
            borrowers: 1,
            dirty: false,
            nodes: Nodes::default(),
        })
    })
}

pub(in crate::bridge) fn open_manifest(component: usize) -> Result<u32, String> {
    open(Source::Manifest { component }, None)
}

pub(in crate::bridge) fn with_read<R>(
    handle: u32,
    f: impl FnOnce(&mut Nodes, &AxmlDocument) -> Result<R, String>,
) -> Result<R, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs.get_mut(handle).ok_or_else(|| not_open(handle))?;
        match open.source {
            Source::Manifest { component } => crate::bridge::phase::with_ctx_result(|ctx| {
                let component = ctx
                    .apk()
                    .component(component)
                    .ok_or("missing APK component")?;
                f(&mut open.nodes, component.manifest())
            }),
            _ => f(
                &mut open.nodes,
                open.owned
                    .as_ref()
                    .expect("non-manifest documents own their model"),
            ),
        }
    })
}

pub(in crate::bridge) fn empty_document() -> AxmlDocument {
    AxmlDocument::new(StringEncoding::Utf8)
}

pub(in crate::bridge) fn with_edit<R>(
    handle: u32,
    f: impl FnOnce(&mut Nodes, &mut AxmlDocument, &mut PatchContext<'_>) -> Result<R, String>,
) -> Result<R, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs.get_mut(handle).ok_or_else(|| not_open(handle))?;
        open.dirty = true;
        crate::bridge::phase::with_ctx_result(|ctx| {
            // Move the model for this operation so resource-scope resolution can borrow
            // the APK. There is one model, restored even when an FFI operation unwinds.
            let mut document = match open.source {
                Source::Manifest { component } => std::mem::replace(
                    ctx.component_mut(component)
                        .map_err(|e| e.to_string())?
                        .manifest_mut(),
                    empty_document(),
                ),
                _ => open
                    .owned
                    .take()
                    .expect("non-manifest documents own their model"),
            };
            let outcome =
                panic::catch_unwind(AssertUnwindSafe(|| f(&mut open.nodes, &mut document, ctx)));
            match open.source {
                Source::Manifest { component } => {
                    *ctx.component_mut(component)
                        .expect("component remains in the APK during XML edits")
                        .manifest_mut() = document;
                }
                _ => open.owned = Some(document),
            }
            match outcome {
                Ok(result) => result,
                Err(payload) => panic::resume_unwind(payload),
            }
        })
    })
}

pub(in crate::bridge) fn edit_manifest<R>(
    ctx: &mut PatchContext<'_>,
    component: usize,
    edit: impl FnOnce(&mut AxmlDocument, &mut Nodes) -> R,
) -> R {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs
            .iter_mut()
            .map(|(_, open)| open)
            .find(|open| open.source == Source::Manifest { component });
        let mut unused = Nodes::default();
        let nodes = match open {
            Some(open) => {
                open.dirty = true;
                &mut open.nodes
            }
            None => &mut unused,
        };
        edit(
            ctx.apk_mut()
                .component_mut(component)
                .expect("component index comes from component_index")
                .manifest_mut(),
            nodes,
        )
    })
}

#[export]
pub fn xml_open(component_name: Option<String>, path: String) -> Option<u32> {
    checked(with_component(component_name, |ctx, component| {
        if path == reseam_apk::entry::MANIFEST_ENTRY {
            return open_manifest(component).map(Some);
        }
        let source = Source::File {
            component,
            path: path.clone(),
        };
        let existing =
            DOCS.with(|docs| docs.borrow().iter().any(|(_, open)| open.source == source));
        if existing {
            return open(source, None).map(Some);
        }
        let Some(bytes) = ctx
            .read_file(component, &path)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let document = AxmlDocument::parse(&bytes).map_err(|error| error.to_string())?;
        open(source, Some(document)).map(Some)
    }))
}

#[export]
pub fn xml_close(handle: u32) {
    let result = DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs.get_mut(handle).ok_or_else(|| not_open(handle))?;
        open.borrowers -= 1;
        if open.borrowers != 0 {
            return Ok(());
        }
        let open = docs.remove(handle).expect("the document is open");
        if open.dirty
            && let Source::File { component, path } = open.source
        {
            let bytes = open
                .owned
                .expect("files own their XML model")
                .serialize()
                .map_err(|e| e.to_string())?;
            crate::bridge::phase::try_with_ctx(|ctx| {
                ctx.inject_file(component, &path, bytes, Compression::Deflated)
            })
            .flatten()
            .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    });
    checked(result);
}
