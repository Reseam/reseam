// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use reseam_model::analysis::{
    CallNode, CallStop, CallTree, CodeSite, Member, MethodRef, RefTarget, Relation,
};

use super::Analysis;
use super::refs::callers;
use crate::error::Result;

impl Analysis {
    /// The callers of `method`, their callers, and so on `depth` levels up.
    /// A branch stops early at a method nothing in the app calls, at one called
    /// only through a shared interface method, or at one already on its path.
    pub fn callers(&mut self, method: &MethodRef, depth: u32) -> Result<CallTree> {
        let mut nodes = Vec::new();
        let (direct, stop) = self.callers_of(method)?;
        self.climb(direct, 1, depth, &mut vec![method.clone()], &mut nodes)?;
        Ok(CallTree {
            root: method.clone(),
            stop,
            nodes,
        })
    }

    /// The methods calling `method`, or why there are none.
    fn callers_of(
        &mut self,
        method: &MethodRef,
    ) -> Result<(BTreeMap<MethodRef, CodeSite>, Option<CallStop>)> {
        let target = RefTarget::Member(Member::Method(method.clone()));
        let references = self.refs(&target)?;
        let callers = callers(references.sites);
        if !callers.is_empty() {
            return Ok((callers, None));
        }
        let shared = references
            .relations
            .into_iter()
            .find_map(|relation| match relation {
                Relation::Shared {
                    method,
                    implementers,
                } => Some(CallStop::Shared {
                    through: method,
                    implementers,
                }),
                _ => None,
            });
        let stop = shared.unwrap_or_else(|| CallStop::NoCallers {
            external_supertypes: self.hierarchy().external_supertypes(&method.class),
        });
        Ok((callers, Some(stop)))
    }

    fn climb(
        &mut self,
        callers: BTreeMap<MethodRef, CodeSite>,
        level: u32,
        depth: u32,
        path: &mut Vec<MethodRef>,
        nodes: &mut Vec<CallNode>,
    ) -> Result<()> {
        for (caller, site) in callers {
            if path.contains(&caller) {
                nodes.push(CallNode {
                    depth: level,
                    site,
                    stop: Some(CallStop::Cycle),
                });
                continue;
            }
            let (above, stop) = self.callers_of(&caller)?;
            nodes.push(CallNode {
                depth: level,
                site,
                stop,
            });
            if !above.is_empty() && level < depth {
                path.push(caller);
                self.climb(above, level + 1, depth, path, nodes)?;
                path.pop();
            }
        }
        Ok(())
    }
}
