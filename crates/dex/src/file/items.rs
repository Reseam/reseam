// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexBytes;
use crate::error::{Result, invalid, invalid_call_site, invalid_method_handle_type, require_array};
use crate::read::{read_u16, read_u32};
use crate::types::encoded_value::EncodedValue;
use crate::types::header::ParseOptions;
use crate::types::method_handle::{
    CallSiteItem, MethodHandle, MethodHandleMember, MethodHandleType,
};
use crate::types::{FieldIdx, MethodIdx};
use std::borrow::Cow;

/// A record whose encoded representation may reference variable-size data.
pub trait FileRecord: Clone {
    const SIZE: usize;
    fn read(buf: &[u8], offset: usize, index: u32, options: ParseOptions) -> Result<Self>;
}

/// File-backed records with appended owned entries. Reads borrow authored entries
/// and decode only one file entry; indices remain stable when appending.
#[derive(Debug, Clone)]
pub struct FileTable<T> {
    raw: Option<DexBytes>,
    offset: usize,
    count: usize,
    options: ParseOptions,
    tail: Vec<T>,
}

impl<T> Default for FileTable<T> {
    fn default() -> Self {
        Self {
            raw: None,
            offset: 0,
            count: 0,
            options: ParseOptions::default(),
            tail: Vec::new(),
        }
    }
}

impl<T: FileRecord> FileTable<T> {
    pub(crate) fn from_raw(
        raw: DexBytes,
        offset: u32,
        count: u32,
        options: ParseOptions,
    ) -> Result<Self> {
        require_array(
            raw.as_bytes(),
            offset as usize,
            count as usize,
            T::SIZE,
            "data table",
        )?;
        let table = Self {
            raw: Some(raw),
            offset: offset as usize,
            count: count as usize,
            options,
            tail: Vec::new(),
        };
        Ok(table)
    }
    pub fn len(&self) -> usize {
        self.count + self.tail.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Returns an error for a missing index or malformed referenced data.
    pub fn get(&self, index: usize) -> Result<Cow<'_, T>> {
        if index < self.count {
            let raw = self.raw.as_ref().expect("file entries retain their source");
            T::read(
                raw.as_bytes(),
                self.offset + index * T::SIZE,
                index as u32,
                self.options,
            )
            .map(Cow::Owned)
        } else {
            self.tail
                .get(index - self.count)
                .map(Cow::Borrowed)
                .ok_or_else(|| {
                    invalid(
                        "data table",
                        format!("index {index} exceeds {} records", self.len()),
                    )
                })
        }
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Result<Cow<'_, T>>> {
        (0..self.len()).map(|index| self.get(index))
    }
    pub fn push(&mut self, record: T) -> usize {
        let index = self.len();
        self.tail.push(record);
        index
    }
}

impl FileRecord for MethodHandle {
    const SIZE: usize = 8;
    fn read(buf: &[u8], offset: usize, _: u32, _: ParseOptions) -> Result<Self> {
        require_array(buf, offset, 1, Self::SIZE, "method handle")?;
        let value = read_u16(buf, offset)?;
        let handle_type =
            MethodHandleType::from_u16(value).ok_or_else(|| invalid_method_handle_type(value))?;
        let index = u32::from(read_u16(buf, offset + 4)?);
        let member = if handle_type.is_field() {
            MethodHandleMember::Field(FieldIdx(index))
        } else {
            MethodHandleMember::Method(MethodIdx(index))
        };
        Ok(Self {
            handle_type,
            member,
        })
    }
}

impl FileRecord for CallSiteItem {
    const SIZE: usize = 4;
    fn read(buf: &[u8], offset: usize, index: u32, options: ParseOptions) -> Result<Self> {
        let offset = read_u32(buf, offset)? as usize;
        let (mut values, _) =
            crate::read::encoded_value::read_encoded_array_with_opts(buf, offset, options)?;
        let [
            EncodedValue::MethodHandle(bootstrap_method),
            EncodedValue::String(method_name),
            EncodedValue::MethodType(method_type),
            ..,
        ] = values.as_slice()
        else {
            return Err(invalid_call_site(
                index,
                "expected a method handle, name, and method type",
            ));
        };
        let (bootstrap_method, method_name) = (*bootstrap_method, *method_name);
        let method_type = *method_type;
        let extra_arguments = values.split_off(3);
        Ok(Self {
            bootstrap_method,
            method_name,
            method_type,
            extra_arguments,
        })
    }
}
