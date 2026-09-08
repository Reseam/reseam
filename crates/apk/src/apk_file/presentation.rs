// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application's label and icon as a launcher shows them, resolved
//! through the resource tables without a platform resource loader.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::HashSet;

use super::ApkFile;
use crate::axml::android_attrs::{ATTR_DRAWABLE, ATTR_ICON, ATTR_LABEL};
use crate::axml::AxmlDocument;
use crate::error::Result;
use crate::ResValue;

const BITMAP_EXTENSIONS: [&str; 4] = [".png", ".webp", ".jpg", ".jpeg"];

/// A launcher icon as the manifest declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationIcon {
    /// An encoded PNG, WebP or JPEG.
    Bitmap(Vec<u8>),
    /// Layers on a 108dp canvas of which a launcher shows the central 72dp.
    Adaptive {
        background: IconLayer,
        foreground: IconLayer,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconLayer {
    Bitmap(Vec<u8>),
    /// ARGB.
    Color(u32),
}

/// What a resource attribute finally resolves to in one configuration.
struct Resolved {
    value: Leaf,
    default_config: bool,
    density: u16,
}

enum Leaf {
    Text(String),
    Color(u32),
}

impl ApkFile {
    /// The default-configuration label, so it is the same on every host. A
    /// localized label needs the platform's resource loader.
    pub fn application_label(&mut self) -> Result<Option<String>> {
        let labels = self.application_attribute(ATTR_LABEL)?;
        Ok(labels
            .iter()
            .find(|label| label.default_config)
            .or(labels.first())
            .and_then(|label| match &label.value {
                Leaf::Text(text) => Some(text.clone()),
                Leaf::Color(_) => None,
            }))
    }

    /// The densest bitmap the manifest icon resolves to, else its adaptive
    /// icon when both layers are bitmaps or colors. Vector layers need the
    /// platform to render them.
    pub fn application_icon(&mut self) -> Result<Option<ApplicationIcon>> {
        let icons = self.application_attribute(ATTR_ICON)?;
        if let Some(bitmap) = self.densest_bitmap(&icons)? {
            return Ok(Some(ApplicationIcon::Bitmap(bitmap)));
        }
        let Some(bytes) = icons
            .iter()
            .find_map(|icon| match &icon.value {
                Leaf::Text(path) if path.ends_with(".xml") => Some(path.clone()),
                _ => None,
            })
            .map(|path| self.read_entry(&path))
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        let drawable = AxmlDocument::parse(&bytes)?;
        if drawable
            .root()
            .and_then(|root| drawable.element_name(root))
            .as_deref()
            != Some("adaptive-icon")
        {
            return Ok(None);
        }
        let background = self.adaptive_layer(&drawable, "background")?;
        let foreground = self.adaptive_layer(&drawable, "foreground")?;
        Ok(background
            .zip(foreground)
            .map(|(background, foreground)| ApplicationIcon::Adaptive {
                background,
                foreground,
            }))
    }

    fn adaptive_layer(&mut self, drawable: &AxmlDocument, name: &str) -> Result<Option<IconLayer>> {
        let Some(attribute) = drawable
            .find_element(name)
            .and_then(|element| drawable.attribute(element, ATTR_DRAWABLE))
        else {
            return Ok(None);
        };
        let literal = drawable.attribute_string(attribute).map(Cow::into_owned);
        let resolved = self.resolve(attribute.value, literal)?;
        let Some(bitmap) = self.densest_bitmap(&resolved)? else {
            return Ok(resolved.iter().find_map(|entry| match entry.value {
                Leaf::Color(color) => Some(IconLayer::Color(color)),
                Leaf::Text(_) => None,
            }));
        };
        Ok(Some(IconLayer::Bitmap(bitmap)))
    }

    fn application_attribute(&mut self, attr: u32) -> Result<Vec<Resolved>> {
        let manifest = self.base().manifest();
        let Some(attribute) = manifest
            .find_element("application")
            .and_then(|application| manifest.attribute(application, attr))
        else {
            return Ok(Vec::new());
        };
        let literal = manifest.attribute_string(attribute).map(Cow::into_owned);
        self.resolve(attribute.value, literal)
    }

    /// Everything an attribute value stands for: the literal itself, or
    /// each configuration's value of the referenced resource.
    fn resolve(&mut self, value: ResValue, literal: Option<String>) -> Result<Vec<Resolved>> {
        let mut resolved = Vec::new();
        if value.kind == ResValue::REFERENCE {
            self.resolve_reference(value.data, &mut HashSet::new(), &mut resolved)?;
        } else if let Some(leaf) = literal.map(Leaf::Text).or(value.color().map(Leaf::Color)) {
            resolved.push(Resolved {
                value: leaf,
                default_config: true,
                density: 0,
            });
        }
        Ok(resolved)
    }

    fn resolve_reference(
        &mut self,
        res_id: u32,
        visited: &mut HashSet<u32>,
        resolved: &mut Vec<Resolved>,
    ) -> Result<()> {
        if !visited.insert(res_id) {
            return Ok(());
        }
        let mut references = Vec::new();
        for component in &mut self.components {
            let Some(table) = component.resources()? else {
                continue;
            };
            for (config, value) in table.values(res_id) {
                let leaf = if value.kind == ResValue::REFERENCE {
                    references.push(value.data);
                    continue;
                } else if value.kind == ResValue::STRING {
                    table
                        .get_string(value.data)
                        .map(|text| Leaf::Text(text.into_owned()))
                } else {
                    value.color().map(Leaf::Color)
                };
                resolved.extend(leaf.map(|value| Resolved {
                    value,
                    default_config: config.is_default_config(),
                    density: config.density(),
                }));
            }
        }
        for reference in references {
            self.resolve_reference(reference, visited, resolved)?;
        }
        Ok(())
    }

    fn densest_bitmap(&mut self, resolved: &[Resolved]) -> Result<Option<Vec<u8>>> {
        let mut bitmaps: Vec<_> = resolved
            .iter()
            .filter_map(|entry| match &entry.value {
                Leaf::Text(path)
                    if BITMAP_EXTENSIONS
                        .iter()
                        .any(|extension| path.ends_with(extension)) =>
                {
                    Some((entry.density, path))
                }
                _ => None,
            })
            .collect();
        bitmaps.sort_by_key(|(density, _)| Reverse(*density));
        for (_, path) in bitmaps {
            if let Some(bytes) = self.read_entry(path)? {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}
