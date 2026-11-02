// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::analysis::{BodyDiff, BodyLine, Change, DiffLine, MethodView};

/// `left` and `right` aligned on their longest common run of lines: what
/// stayed, what `right` added and what it removed.
pub fn diff(left: &MethodView, right: &MethodView) -> BodyDiff {
    let (a, b) = (&left.lines, &right.lines);
    let mut common = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            common[i][j] = if a[i].text == b[j].text {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut lines = Vec::with_capacity(a.len().max(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i].text == b[j].text {
            lines.push(line(Change::Same, Some(&a[i]), Some(&b[j])));
            (i, j) = (i + 1, j + 1);
        } else if j < b.len() && (i == a.len() || common[i][j + 1] >= common[i + 1][j]) {
            lines.push(line(Change::Added, None, Some(&b[j])));
            j += 1;
        } else {
            lines.push(line(Change::Removed, Some(&a[i]), None));
            i += 1;
        }
    }
    let count = |change| {
        lines
            .iter()
            .filter(|line: &&DiffLine| line.change == change)
            .count()
    };
    BodyDiff {
        added: count(Change::Added),
        removed: count(Change::Removed),
        lines,
    }
}

fn line(change: Change, left: Option<&BodyLine>, right: Option<&BodyLine>) -> DiffLine {
    DiffLine {
        change,
        left: left.and_then(|line| line.index),
        right: right.and_then(|line| line.index),
        text: right
            .or(left)
            .expect("a diff line is on at least one side")
            .text
            .clone(),
    }
}
