// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::AxmlDocument;
use reseam_apk::axml::android_attrs::{ATTR_NAME, ATTR_TARGET_ACTIVITY};
use reseam_apk::axml::children;
use reseam_model::analysis::{ClassRef, ComponentKind};

/// An activity, alias, service, receiver or provider the manifest declares.
pub(super) struct Component {
    pub(super) kind: ComponentKind,
    /// The class that runs it; an alias runs its target activity.
    pub(super) class: ClassRef,
    /// The event that starts its element.
    pub(super) element: usize,
    /// Whether an intent filter makes it the launcher entry.
    pub(super) launcher: bool,
}

pub(super) fn components(manifest: &AxmlDocument) -> Vec<Component> {
    let Some(application) = manifest.find_element("application") else {
        return Vec::new();
    };
    children(manifest.events(), application)
        .into_iter()
        .filter_map(|element| {
            let kind = match manifest.element_name(element)?.as_ref() {
                "activity" => ComponentKind::Activity,
                "activity-alias" => ComponentKind::ActivityAlias,
                "service" => ComponentKind::Service,
                "receiver" => ComponentKind::Receiver,
                "provider" => ComponentKind::Provider,
                _ => return None,
            };
            let runs = match kind {
                ComponentKind::ActivityAlias => ATTR_TARGET_ACTIVITY,
                _ => ATTR_NAME,
            };
            let class = class_attribute(manifest, element, runs)?;
            Some(Component {
                kind,
                class,
                element,
                launcher: is_launcher(manifest, element),
            })
        })
        .collect()
}

pub(super) fn application(manifest: &AxmlDocument) -> Option<ClassRef> {
    class_attribute(manifest, manifest.find_element("application")?, ATTR_NAME)
}

fn class_attribute(manifest: &AxmlDocument, element: usize, attribute: u32) -> Option<ClassRef> {
    let name = manifest.attribute_string(manifest.attribute(element, attribute)?)?;
    ClassRef::parse_any(&manifest.qualified_class_name(&name)).ok()
}

/// Whether an intent filter of `element` takes the `MAIN` action in the `LAUNCHER` category.
fn is_launcher(manifest: &AxmlDocument, element: usize) -> bool {
    let events = manifest.events();
    children(events, element).into_iter().any(|filter| {
        if manifest.element_name(filter).as_deref() != Some("intent-filter") {
            return false;
        }
        let names: Vec<String> = children(events, filter)
            .into_iter()
            .filter_map(|child| {
                let name = manifest.attribute(child, ATTR_NAME)?;
                manifest
                    .attribute_string(name)
                    .map(std::borrow::Cow::into_owned)
            })
            .collect();
        names
            .iter()
            .any(|name| name == "android.intent.action.MAIN")
            && names
                .iter()
                .any(|name| name == "android.intent.category.LAUNCHER")
    })
}
