// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::handles::checked;
use boltffi::export;
use reseam_apk::ResourceScope;
use reseam_apk::axml::{self, AxmlDocument, AxmlEvent, NodeMetadata};

use super::xml_attributes::attribute_text;
use super::xml_documents::{Source, open, with_edit, with_read};
pub(super) use super::xml_documents::{edit_manifest, finish, open_manifest, reset};
pub(super) use super::xml_nodes::Nodes;
use super::xml_nodes::{ElementId, Position, Tree};

#[export]
pub fn xml_root(doc: u32) -> u32 {
    checked(with_read(doc, |nodes, document| {
        nodes
            .id(Position {
                tree: Tree::Document,
                index: document.root().ok_or("XML document has no root")?,
            })
            .map(|id| id.0)
    }))
}

#[export]
pub fn xml_find_by_tag(doc: u32, tag: String) -> Vec<u32> {
    checked(with_read(doc, |nodes, document| {
        (0..document.events().len())
            .filter(|&index| document.element_name(index).as_deref() == Some(tag.as_str()))
            .map(|index| {
                nodes
                    .id(Position {
                        tree: Tree::Document,
                        index,
                    })
                    .map(|id| id.0)
            })
            .collect()
    }))
}

#[export]
pub fn xml_find_by_attribute(doc: u32, name: String, value: String) -> Vec<u32> {
    checked(with_read(doc, |nodes, document| {
        document.events().iter().enumerate().filter(|(_, event)| matches!(event, AxmlEvent::StartElement { attributes, .. } if attribute_text(document, attributes, &name).as_deref() == Some(value.as_str()))).map(|(index, _)| nodes.id(Position { tree: Tree::Document, index }).map(|id| id.0)).collect()
    }))
}

#[export]
pub fn xml_children(doc: u32, element: u32) -> Vec<u32> {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        let end = nodes.end(document, position)?;
        let mut depth = 0usize;
        let children: Vec<_> = nodes
            .events(document, position.tree)?
            .iter()
            .enumerate()
            .take(end)
            .skip(position.index + 1)
            .filter_map(|(index, event)| match event {
                AxmlEvent::StartElement { .. } => {
                    let direct = depth == 0;
                    depth += 1;
                    direct.then_some(index)
                }
                AxmlEvent::EndElement { .. } => {
                    depth -= 1;
                    None
                }
                _ => None,
            })
            .collect();
        children
            .into_iter()
            .map(|index| {
                nodes
                    .id(Position {
                        tree: position.tree,
                        index,
                    })
                    .map(|id| id.0)
            })
            .collect()
    }))
}

#[export]
pub fn xml_parent(doc: u32, element: u32) -> Option<u32> {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        let events = nodes.events(document, position.tree)?;
        let mut depth = 0usize;
        let parent = (0..position.index)
            .rev()
            .find(|&index| match events[index] {
                AxmlEvent::EndElement { .. } => {
                    depth += 1;
                    false
                }
                AxmlEvent::StartElement { .. } if depth == 0 => true,
                AxmlEvent::StartElement { .. } => {
                    depth -= 1;
                    false
                }
                _ => false,
            });
        parent
            .map(|index| {
                nodes
                    .id(Position {
                        tree: position.tree,
                        index,
                    })
                    .map(|id| id.0)
            })
            .transpose()
    }))
}

#[export]
pub fn xml_tag_name(doc: u32, element: u32) -> String {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        let Some(AxmlEvent::StartElement { name, .. }) =
            nodes.events(document, position.tree)?.get(position.index)
        else {
            return Err(format!("element {element} is absent"));
        };
        document
            .string(*name)
            .map(std::borrow::Cow::into_owned)
            .ok_or_else(|| "invalid XML element name".into())
    }))
}

#[export]
pub fn xml_create_element(doc: u32, tag: String) -> u32 {
    checked(
        with_edit(doc, |nodes, document, _| {
            let name = document.intern_string(&tag);
            nodes.pending(vec![
                AxmlEvent::StartElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                    attributes: Vec::new(),
                },
                AxmlEvent::EndElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                },
            ])
        })
        .map(|id| id.0),
    )
}

#[export]
pub fn xml_append_child(doc: u32, parent: u32, child: u32) -> Result<(), String> {
    let parent = ElementId(parent);
    let child = ElementId(child);
    with_edit(doc, |nodes, document, _| {
        let parent_position = nodes.position(parent)?;
        let child_position = nodes.position(child)?;
        let child_end = nodes.end(document, child_position)?;
        nodes.end(document, parent_position)?;
        if parent_position.tree == child_position.tree
            && (child_position.index..=child_end).contains(&parent_position.index)
        {
            return Err("an XML element cannot contain itself".into());
        }
        nodes.detach(document, child)?;
        let parent_position = nodes.position(parent)?;
        let index = nodes.end(document, parent_position)?;
        nodes.attach(
            document,
            child,
            Position {
                tree: parent_position.tree,
                index,
            },
        )
    })
}

#[export]
pub fn xml_insert_before(doc: u32, child: u32, before: u32) -> Result<u32, String> {
    let child = ElementId(child);
    let before = ElementId(before);
    with_edit(doc, |nodes, document, _| {
        let target = nodes.position(before)?;
        if target.tree != Tree::Document {
            return Err("insertBefore requires an attached anchor".into());
        }
        let child_position = nodes.position(child)?;
        let end = nodes.end(document, child_position)?;
        if child_position.tree == target.tree
            && (child_position.index..=end).contains(&target.index)
        {
            return Err("an XML element cannot be inserted into itself".into());
        }
        nodes.detach(document, child)?;
        nodes.attach(document, child, nodes.position(before)?)?;
        Ok(child.0)
    })
}

#[export]
pub fn xml_remove_element(doc: u32, element: u32) {
    let element = ElementId(element);
    checked(with_edit(doc, |nodes, document, _| {
        nodes.detach(document, element)?;
        nodes.detached.remove(&element);
        for position in &mut nodes.positions {
            if position.is_some_and(|position| position.tree == Tree::Detached(element)) {
                *position = None;
            }
        }
        nodes.reindex();
        Ok(())
    }));
}

#[export]
pub fn xml_compile(text: String) -> Result<u32, String> {
    let document = super::handles::try_with_ctx(|ctx| {
        ctx.apk_mut()
            .with_resource_scope(0, |scope| axml::build_document(&text, scope))
    })
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    open(Source::Memory, Some(document))
}

#[export]
pub fn xml_declare_namespace(doc: u32, prefix: String, uri: String) -> Result<(), String> {
    with_edit(doc, |nodes, document, _| {
        let before = document.events().len();
        document
            .declare_namespace(&prefix, &uri)
            .map_err(|e| e.to_string())?;
        if document.events().len() != before {
            nodes.inserted_document(0, 1);
        }
        Ok(())
    })
}

#[export]
pub fn xml_adopt(doc: u32, source: u32, element: u32) -> Result<u32, String> {
    let element = ElementId(element);
    if source == doc {
        return Err("use clone within one XML document".into());
    }
    let snapshot = with_read(source, |nodes, document| {
        let position = nodes.position(element)?;
        let end = nodes.end(document, position)?;
        // StringPool clones share the input backing; only the requested subtree is copied.
        AxmlDocument::from_parts(
            document.string_pool().clone(),
            Vec::new(),
            nodes.events(document, position.tree)?[position.index..=end].to_vec(),
        )
        .map_err(|e| e.to_string())
    })?;
    with_edit(doc, |nodes, document, ctx| {
        let events = ctx
            .apk_mut()
            .with_resource_scope(0, |scope| {
                document.adopt(
                    &snapshot,
                    snapshot.events(),
                    scope.as_deref().map(ResourceScope::table),
                )
            })
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        nodes.pending(events)
    })
    .map(|id| id.0)
}

#[export]
pub fn xml_clone_element(doc: u32, element: u32, deep: bool) -> u32 {
    let element = ElementId(element);
    checked(
        with_read(doc, |nodes, document| {
            let position = nodes.position(element)?;
            let events = nodes.events(document, position.tree)?;
            let end = nodes.end(document, position)?;
            let events = if deep {
                events[position.index..=end].to_vec()
            } else {
                vec![events[position.index].clone(), events[end].clone()]
            };
            nodes.pending(events)
        })
        .map(|id| id.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::PatchContext;
    use crate::kotlin::handles::{ContextGuard, RunGuard, check_invocation};
    use crate::kotlin::manifest;
    use crate::kotlin::xml_attributes::{xml_get_attribute, xml_set_attribute};
    use crate::kotlin::xml_documents::{xml_close, xml_open};
    use reseam_apk::Compression;

    #[test]
    fn manifest_views_share_borrows_and_keep_element_identities() {
        let (dir, mut apk) = crate::test_support::apk();
        let mut ctx = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard = ContextGuard::enter(&mut ctx, dir.path().to_owned()).unwrap();
        let outer = manifest::manifest_get_document(None).unwrap();
        let application = xml_find_by_tag(outer, "application".into())[0];
        let main = xml_find_by_attribute(outer, "android:name".into(), ".Main".into())[0];
        let nested = xml_open(None, "AndroidManifest.xml".into()).unwrap();
        let root = xml_root(nested);
        xml_set_attribute(nested, root, "android:versionName".into(), "3.0".into()).unwrap();
        assert_eq!(
            manifest::manifest_version_name(None).as_deref(),
            Some("3.0")
        );
        xml_close(nested);
        manifest::manifest_set_version_name(None, "4.0".into());
        assert_eq!(
            xml_get_attribute(outer, root, "android:versionName".into()).as_deref(),
            Some("4.0")
        );
        manifest::manifest_add_permission(None, "android.permission.INTERNET".into());
        manifest::manifest_set_activity_config_changes(
            None,
            ".Main".into(),
            "orientation|screenSize|unknown".into(),
        );
        assert_eq!(
            xml_get_attribute(outer, main, "android:configChanges".into()).as_deref(),
            Some("1152")
        );
        manifest::manifest_add_intent_filter(
            None,
            ".Main".into(),
            Some("android.intent.action.VIEW".into()),
            None,
            None,
        );
        manifest::manifest_add_activity_alias(
            None,
            ".Main".into(),
            ".Alias".into(),
            false,
            Some("alias".into()),
        );
        manifest::manifest_copy_intent_filters(None, ".Main".into(), ".Alias".into());
        xml_declare_namespace(outer, "test".into(), "urn:test".into()).unwrap();
        assert_eq!(xml_tag_name(outer, application), "application");
        assert_eq!(
            xml_get_attribute(outer, main, "android:name".into()).as_deref(),
            Some(".Main")
        );
        assert_eq!(xml_parent(outer, main), Some(application));
        let alias = xml_find_by_attribute(outer, "android:name".into(), ".Alias".into())[0];
        assert_eq!(
            xml_children(outer, alias)
                .iter()
                .map(|child| xml_tag_name(outer, *child))
                .collect::<Vec<_>>(),
            ["intent-filter"]
        );
        xml_close(outer);
        guard.finish().unwrap();
        assert_eq!(apk.base().manifest().version_name().as_deref(), Some("4.0"));
    }

    #[test]
    fn subtree_handles_follow_moves_and_removal_without_consuming_children() {
        let (dir, mut apk) = crate::test_support::apk();
        let mut ctx = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard = ContextGuard::enter(&mut ctx, dir.path().to_owned()).unwrap();
        let document = manifest::manifest_get_document(None).unwrap();
        let application = xml_find_by_tag(document, "application".into())[0];
        let parent = xml_create_element(document, "activity".into());
        let filter = xml_create_element(document, "intent-filter".into());
        let action = xml_create_element(document, "action".into());
        xml_set_attribute(
            document,
            action,
            "android:name".into(),
            "test.action".into(),
        )
        .unwrap();
        xml_append_child(document, filter, action).unwrap();
        xml_append_child(document, parent, filter).unwrap();
        xml_append_child(document, application, parent).unwrap();
        assert_eq!(xml_parent(document, action), Some(filter));
        assert_eq!(xml_parent(document, filter), Some(parent));
        let first = xml_children(document, application)[0];
        assert_eq!(xml_insert_before(document, parent, first).unwrap(), parent);
        assert_eq!(xml_children(document, application)[0], parent);
        assert_eq!(
            xml_get_attribute(document, action, "android:name".into()).as_deref(),
            Some("test.action")
        );
        let cloned = xml_clone_element(document, parent, false);
        assert!(xml_children(document, cloned).is_empty());
        xml_append_child(document, application, cloned).unwrap();
        xml_remove_element(document, parent);
        assert_eq!(xml_tag_name(document, first), "activity");
        assert_eq!(xml_tag_name(document, cloned), "activity");
        assert!(!xml_children(document, application).contains(&parent));
        xml_close(document);
        check_invocation().unwrap();
        guard.finish().unwrap();
    }

    #[test]
    fn file_edits_commit_on_last_close_and_unfinished_edits_fail() {
        for close in [true, false] {
            let (dir, mut apk) = crate::test_support::apk();
            let mut ctx = PatchContext::new(&mut apk);
            ctx.inject_file(0, "res/layout/main.xml", br#"<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"><TextView android:text="original"/></LinearLayout>"#.to_vec(), Compression::Deflated).unwrap();
            let _run = RunGuard::enter().unwrap();
            let guard = ContextGuard::enter(&mut ctx, dir.path().to_owned()).unwrap();
            let outer = xml_open(None, "res/layout/main.xml".into()).unwrap();
            let nested = xml_open(None, "res/layout/main.xml".into()).unwrap();
            let label = xml_find_by_tag(outer, "TextView".into())[0];
            let source = xml_compile(r#"<TextView xmlns:android="http://schemas.android.com/apk/res/android" android:text="adopted"/>"#.into()).unwrap();
            let adopted = xml_adopt(outer, source, xml_root(source)).unwrap();
            xml_append_child(outer, xml_root(outer), adopted).unwrap();
            xml_close(source);
            assert_eq!(
                xml_get_attribute(outer, adopted, "android:text".into()).as_deref(),
                Some("adopted")
            );
            xml_set_attribute(outer, label, "android:text".into(), "updated".into()).unwrap();
            xml_close(nested);
            assert_eq!(
                xml_get_attribute(outer, label, "android:text".into()).as_deref(),
                Some("updated")
            );
            if close {
                xml_close(outer);
            }
            assert_eq!(check_invocation().is_ok(), close);
            assert_eq!(guard.finish().is_ok(), close);
            let bytes = ctx.read_file(0, "res/layout/main.xml").unwrap().unwrap();
            let written = AxmlDocument::parse(&bytes).unwrap();
            let label = written.find_element("TextView").unwrap();
            assert_eq!(
                written
                    .attribute_named(label, "text")
                    .and_then(|value| written.attribute_string(value))
                    .as_deref(),
                Some(if close { "updated" } else { "original" })
            );
        }
    }
}
