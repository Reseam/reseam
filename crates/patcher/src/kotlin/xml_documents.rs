// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::files::with_component;
use super::handles::{HandleSpace, checked};
use crate::context::PatchContext;
use boltffi::export;
use reseam_apk::axml::AxmlDocument;
use reseam_apk::{Compression, StringEncoding, StringPool};
use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

use super::xml_nodes::Nodes;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Source {
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

#[derive(Default)]
struct Documents {
    identities: HandleSpace,
    slots: Vec<Option<OpenDoc>>,
}

impl Documents {
    fn slot_mut(&mut self, handle: u32) -> Result<&mut Option<OpenDoc>, String> {
        self.identities
            .slot(handle)
            .and_then(|slot| self.slots.get_mut(slot))
            .ok_or_else(|| format!("invalid XML document {handle}"))
    }
}

thread_local! { static DOCS: RefCell<Documents> = RefCell::new(Documents::default()); }

pub(super) fn reset() {
    DOCS.with(|docs| *docs.borrow_mut() = Documents::default());
}

pub(super) fn finish() -> Result<(), String> {
    DOCS.with(|docs| {
        let docs = docs.borrow();
        if let Some((id, _)) = docs
            .slots
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| slot.as_ref().map(|open| (id, open)))
            .find(|(_, open)| open.dirty && open.source != Source::Memory)
        {
            Err(format!(
                "edited XML document {} has unclosed borrowers",
                docs.identities.handle(id)
            ))
        } else {
            Ok(())
        }
    })
}

pub(super) fn open(source: Source, document: Option<AxmlDocument>) -> Result<u32, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        if source != Source::Memory
            && let Some((id, Some(existing))) = docs
                .slots
                .iter_mut()
                .enumerate()
                .find(|(_, slot)| slot.as_ref().is_some_and(|open| open.source == source))
        {
            existing.borrowers = existing
                .borrowers
                .checked_add(1)
                .ok_or("too many XML borrowers")?;
            return Ok(docs.identities.handle(id));
        }
        let slot = docs.slots.len();
        let id = docs
            .identities
            .allocate(slot)
            .map_err(|error| error.to_string())?;
        docs.slots.push(Some(OpenDoc {
            source,
            owned: document,
            borrowers: 1,
            dirty: false,
            nodes: Nodes::default(),
        }));
        Ok(id)
    })
}

pub(super) fn open_manifest(component: usize) -> Result<u32, String> {
    open(Source::Manifest { component }, None)
}

pub(super) fn with_read<R>(
    handle: u32,
    f: impl FnOnce(&mut Nodes, &AxmlDocument) -> Result<R, String>,
) -> Result<R, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs
            .slot_mut(handle)?
            .as_mut()
            .ok_or_else(|| format!("closed XML document {handle}"))?;
        match open.source {
            Source::Manifest { component } => super::handles::with_ctx_result(|ctx| {
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

pub(super) fn empty_document() -> AxmlDocument {
    AxmlDocument::from_parts(
        StringPool::new(Vec::new(), StringEncoding::Utf8),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty document has no invalid strings")
}

pub(super) fn with_edit<R>(
    handle: u32,
    f: impl FnOnce(&mut Nodes, &mut AxmlDocument, &mut PatchContext<'_>) -> Result<R, String>,
) -> Result<R, String> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs
            .slot_mut(handle)?
            .as_mut()
            .ok_or_else(|| format!("closed XML document {handle}"))?;
        open.dirty = true;
        super::handles::with_ctx_result(|ctx| {
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

pub(super) fn edit_manifest<R>(
    ctx: &mut PatchContext<'_>,
    component: usize,
    edit: impl FnOnce(&mut AxmlDocument, &mut Nodes) -> R,
) -> R {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs
            .slots
            .iter_mut()
            .flatten()
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
                .expect("component validated by with_component")
                .manifest_mut(),
            nodes,
        )
    })
}

#[export]
pub fn xml_open(component: Option<String>, path: String) -> Option<u32> {
    with_component(component, |ctx, component| {
        if path == reseam_apk::entry::MANIFEST_ENTRY {
            return checked(open_manifest(component).map(Some));
        }
        let source = Source::File {
            component,
            path: path.clone(),
        };
        let existing = DOCS.with(|docs| {
            docs.borrow()
                .slots
                .iter()
                .flatten()
                .any(|open| open.source == source)
        });
        if existing {
            return checked(open(source, None).map(Some));
        }
        let bytes = checked(ctx.read_file(component, &path))?;
        let document = checked(AxmlDocument::parse(&bytes).map(Some))?;
        checked(open(source, Some(document)).map(Some))
    })
    .flatten()
}

#[export]
pub fn xml_close(handle: u32) {
    let result = DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let slot = docs.slot_mut(handle)?;
        let mut open = slot
            .take()
            .ok_or_else(|| format!("closed XML document {handle}"))?;
        open.borrowers -= 1;
        if open.borrowers != 0 {
            *slot = Some(open);
            return Ok(());
        }
        if open.dirty
            && let Source::File { component, path } = open.source
        {
            let bytes = open
                .owned
                .expect("files own their XML model")
                .serialize()
                .map_err(|e| e.to_string())?;
            super::handles::try_with_ctx(|ctx| {
                ctx.inject_file(component, &path, bytes, Compression::Deflated)
            })
            .flatten()
            .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    });
    checked(result);
}
