// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::annotation::AnnotationsDirectory;
use super::debug::DebugInfo;
use super::header::{Loading, ParseOptions};
use crate::error::Result;
use crate::file::DexBytes;
use std::borrow::Cow;

/// Metadata either owned by the IR or still backed by its original file.
/// Cloning deferred metadata shares the source mapping. Reading it decodes only
/// this item; mutation retains the decoded item and its source identity.
#[derive(Debug, Clone)]
pub struct Metadata<T> {
    state: State<T>,
}

#[derive(Debug, Clone)]
enum State<T> {
    Decoded(T),
    Source {
        source: DexBytes,
        offset: u32,
        options: ParseOptions,
        value: Option<T>,
    },
}

/// The metadata grammars that can remain deferred in a materialized class.
pub trait MetadataItem: Clone {
    fn decode(source: &[u8], offset: u32, options: ParseOptions) -> Result<Self>;
}

impl MetadataItem for DebugInfo {
    fn decode(source: &[u8], offset: u32, options: ParseOptions) -> Result<Self> {
        crate::read::debug::read_debug_info(source, offset, options)
    }
}
impl MetadataItem for AnnotationsDirectory {
    fn decode(source: &[u8], offset: u32, options: ParseOptions) -> Result<Self> {
        crate::read::annotation::read_annotations_directory(source, offset, options)
    }
}

impl<T> Metadata<T> {
    pub fn new(value: T) -> Self {
        Self {
            state: State::Decoded(value),
        }
    }
}

impl<T: MetadataItem> Metadata<T> {
    pub(crate) fn from_source(
        source: &DexBytes,
        offset: u32,
        options: ParseOptions,
        loading: Loading,
    ) -> Result<Self> {
        let value = match loading {
            Loading::Eager => Some(T::decode(source.as_bytes(), offset, options)?),
            Loading::Deferred => None,
        };
        Ok(Self {
            state: State::Source {
                source: source.clone(),
                offset,
                options,
                value,
            },
        })
    }

    pub(crate) fn belongs_to(&self, original: Option<&DexBytes>) -> bool {
        matches!((&self.state, original), (State::Source { source, .. }, Some(original)) if source.same_source(original))
    }

    pub fn read(&self) -> Result<Cow<'_, T>> {
        match &self.state {
            State::Decoded(value)
            | State::Source {
                value: Some(value), ..
            } => Ok(Cow::Borrowed(value)),
            State::Source {
                source,
                offset,
                options,
                value: None,
            } => Ok(Cow::Owned(T::decode(source.as_bytes(), *offset, *options)?)),
        }
    }

    pub fn resolve_mut(&mut self) -> Result<&mut T> {
        match &mut self.state {
            State::Decoded(value) => Ok(value),
            State::Source {
                source,
                offset,
                options,
                value,
            } => {
                if value.is_none() {
                    *value = Some(T::decode(source.as_bytes(), *offset, *options)?);
                }
                Ok(value.as_mut().expect("source metadata was decoded above"))
            }
        }
    }
}

impl<T: Default> Default for Metadata<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: PartialEq> PartialEq for Metadata<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.state, &other.state) {
            (State::Decoded(left), State::Decoded(right)) => left == right,
            (
                State::Source {
                    source: left,
                    offset: a,
                    value: x,
                    ..
                },
                State::Source {
                    source: right,
                    offset: b,
                    value: y,
                    ..
                },
            ) => match (x, y) {
                (Some(x), Some(y)) => x == y,
                (None, None) => a == b && left.same_source(right),
                _ => false,
            },
            (State::Decoded(x), State::Source { value: Some(y), .. })
            | (State::Source { value: Some(x), .. }, State::Decoded(y)) => x == y,
            _ => false,
        }
    }
}
