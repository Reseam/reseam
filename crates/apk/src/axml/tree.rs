// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Element structure over an event stream: a document's, or a detached subtree's.

use super::AxmlEvent;

/// The end event of the element that starts at `start`, by element depth as
/// Android's parser reads it. `None` when `start` is not an element start or
/// the element is unterminated.
pub fn element_end(events: &[AxmlEvent], start: usize) -> Option<usize> {
    if !matches!(events.get(start), Some(AxmlEvent::StartElement { .. })) {
        return None;
    }
    let mut depth = 0usize;
    for (index, event) in events.iter().enumerate().skip(start) {
        match event {
            AxmlEvent::StartElement { .. } => depth += 1,
            AxmlEvent::EndElement { .. } => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The start events of the direct children of the element at `start`.
pub fn children(events: &[AxmlEvent], start: usize) -> Vec<usize> {
    let Some(end) = element_end(events, start) else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut children = Vec::new();
    for (index, event) in events.iter().enumerate().take(end).skip(start + 1) {
        match event {
            AxmlEvent::StartElement { .. } => {
                if depth == 0 {
                    children.push(index);
                }
                depth += 1;
            }
            AxmlEvent::EndElement { .. } => depth -= 1,
            _ => {}
        }
    }
    children
}

/// The start event of the element containing the event at `index`.
pub fn parent(events: &[AxmlEvent], index: usize) -> Option<usize> {
    let mut depth = 0usize;
    (0..index.min(events.len()))
        .rev()
        .find(|&at| match events[at] {
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
        })
}
