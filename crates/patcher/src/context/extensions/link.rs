// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Links extension DEX files into the target APK on demand as patch code
//! references classes outside the app and Android platform.

use tracing::debug;

use crate::context::{ClassLocation, PatchContext, undo};

fn class_of(descriptor: &str) -> Option<&str> {
    let element = descriptor.trim_start_matches('[');
    element.starts_with('L').then_some(element)
}

fn is_platform(descriptor: &str) -> bool {
    const PLATFORM: [&str; 10] = [
        "Ljava/",
        "Ljavax/",
        "Landroid/",
        "Ldalvik/",
        "Llibcore/",
        "Lsun/",
        "Lorg/json/",
        "Lorg/xml/",
        "Lorg/w3c/",
        "Lorg/apache/http/",
    ];
    PLATFORM.iter().any(|prefix| descriptor.starts_with(prefix))
}

impl PatchContext<'_> {
    /// Makes sure every class the descriptors refer to is defined, merging
    /// extension DEX files as needed. A reference nothing defines is logged
    /// once, since it is almost always a typo in the patch.
    pub fn link_types<'s>(&mut self, descriptors: impl IntoIterator<Item = &'s str>) {
        for descriptor in descriptors {
            let Some(class) = class_of(descriptor) else {
                continue;
            };
            if is_platform(class) || self.find_class(class).is_some() {
                continue;
            }
            match self.extensions.provider(class) {
                Some(index) => self.merge_extension(index),
                None => {
                    if self.extensions.warned.insert(class.to_owned()) {
                        self.journal
                            .record(undo::Part::Warned(class.to_owned()), || {
                                undo::Undo::Warned(class.to_owned())
                            });
                        self.log().warn(format!(
                            "{class} is not defined by the app or any extension in the bundle"
                        ));
                    }
                }
            }
        }
    }

    pub fn find_or_link_class(&mut self, descriptor: &str) -> Option<ClassLocation> {
        if let Some(location) = self.find_class(descriptor) {
            return Some(location);
        }
        let index = self.extensions.provider(descriptor)?;
        self.merge_extension(index);
        self.find_class(descriptor)
    }

    fn merge_extension(&mut self, index: usize) {
        let mut pending = vec![index];
        while let Some(index) = pending.pop() {
            let Some(dex) = self.extensions.files[index].file.take() else {
                continue;
            };
            debug!(source = %self.extensions.files[index].source, "linking extension");
            pending.extend(
                dex.types()
                    .iter()
                    .filter_map(|string| {
                        self.extensions
                            .providers
                            .get(dex.strings().get(string).as_ref())
                            .copied()
                    })
                    .filter(|&provider| provider != index),
            );
            let dex_index = self.apk.dex().len();
            self.journal
                .record(undo::Part::Merge(index), || undo::Undo::Merged {
                    extension: index,
                    dex: dex_index,
                });
            self.apk_mut().add_dex(dex);
        }
    }
}
