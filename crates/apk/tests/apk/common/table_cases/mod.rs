// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod edits;
mod lookup;

use edits::{array_case, file_cases, style_case};
use lookup::{extended_case, index_cases, pool_cases, strings_case};
use reseam_apk::{ResValue, ResourceTable};

pub(super) type Read = Box<dyn Fn(&ResourceTable) -> reseam_apk::Result<Readback>>;

pub(super) type Edit = Box<dyn Fn(&mut ResourceTable) -> reseam_apk::Result<()>>;

#[derive(Debug, Default, PartialEq)]
pub struct Readback {
    pub texts: Vec<String>,
    pub numbers: Vec<u32>,
    pub values: Vec<ResValue>,
    pub bytes: Vec<Vec<u8>>,
}

pub struct Case {
    pub input: Vec<u8>,
    pub edit: Edit,
    pub read: Read,
    pub expected: Readback,
}

pub(super) fn text_values(
    table: &ResourceTable,
    names: &[&str],
) -> reseam_apk::Result<Vec<String>> {
    names
        .iter()
        .map(|name| {
            Ok(table
                .string_value(name)?
                .expect("fixture string")
                .into_owned())
        })
        .collect()
}

pub fn cases() -> reseam_apk::Result<Vec<Case>> {
    let mut cases = vec![
        strings_case()?,
        extended_case(),
        style_case()?,
        array_case()?,
    ];
    cases.extend(index_cases());
    cases.extend(file_cases()?);
    cases.extend(pool_cases());
    Ok(cases)
}
