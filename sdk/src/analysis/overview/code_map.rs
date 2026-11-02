// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use reseam_model::analysis::{ClassRef, CodeGroup, PackageGroup};

use super::catalog::LIBRARIES;

/// A class name R8 gave: its own name at most three characters long, as R8
/// names run once one- and two-letter names are used up.
fn is_obfuscated(class: &ClassRef) -> bool {
    class.simple_name().chars().count() <= 3
}

/// Classes grouped by the package that names them, the app's first, then by size.
pub(super) fn code_map<'a>(
    classes: impl Iterator<Item = &'a ClassRef>,
    app_package: Option<&str>,
) -> Vec<PackageGroup> {
    let mut groups: HashMap<CodeGroup, PackageGroup> = HashMap::new();
    for class in classes {
        let group = group_of(&class.package(), app_package);
        let entry = groups.entry(group.clone()).or_insert(PackageGroup {
            group,
            classes: 0,
            obfuscated: 0,
        });
        entry.classes += 1;
        entry.obfuscated += usize::from(is_obfuscated(class));
    }
    let mut groups: Vec<PackageGroup> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        let app = |group: &PackageGroup| !matches!(group.group, CodeGroup::App(_));
        (app(a), std::cmp::Reverse(a.classes), &a.group).cmp(&(
            app(b),
            std::cmp::Reverse(b.classes),
            &b.group,
        ))
    });
    groups
}

fn group_of(package: &str, app_package: Option<&str>) -> CodeGroup {
    let within = |prefix: &str| package == prefix || package.starts_with(&format!("{prefix}."));
    if let Some(app) = app_package.filter(|app| within(app)) {
        return CodeGroup::App(app.to_owned());
    }
    if let Some(library) = LIBRARIES
        .iter()
        .filter(|library| within(library))
        .max_by_key(|library| library.len())
    {
        return CodeGroup::Library((*library).to_owned());
    }
    let mut segments = package.split('.');
    match (segments.next(), segments.next()) {
        (Some(first), None) if first.chars().count() <= 3 => CodeGroup::ShortPackages,
        (Some(first), Some(second)) => CodeGroup::Other(format!("{first}.{second}")),
        (Some(first), None) => CodeGroup::Other(first.to_owned()),
        (None, _) => CodeGroup::ShortPackages,
    }
}

#[cfg(test)]
mod tests {
    use reseam_model::analysis::{ClassRef, CodeGroup};

    use super::code_map;

    #[test]
    fn classes_group_by_app_library_and_short_package() {
        let classes: Vec<ClassRef> = [
            "Lcom/lumen/notes/ui/MainActivity;",
            "Lcom/lumen/notes/a;",
            "Lb7/r;",
            "Lp6/k;",
            "La;",
            "Landroidx/core/app/NotificationCompat;",
            "Lcom/google/android/gms/ads/AdView;",
            "Lio/example/Thing;",
        ]
        .iter()
        .map(|descriptor| descriptor.parse().expect("descriptor"))
        .collect();
        let groups = code_map(classes.iter(), Some("com.lumen.notes"));
        let summary: Vec<(CodeGroup, usize, usize)> = groups
            .into_iter()
            .map(|group| (group.group, group.classes, group.obfuscated))
            .collect();
        assert_eq!(
            summary,
            [
                (CodeGroup::App("com.lumen.notes".into()), 2, 1),
                (CodeGroup::ShortPackages, 3, 3),
                (CodeGroup::Library("androidx".into()), 1, 0),
                (CodeGroup::Library("com.google.android.gms".into()), 1, 0),
                (CodeGroup::Other("io.example".into()), 1, 0),
            ]
        );
    }
}
