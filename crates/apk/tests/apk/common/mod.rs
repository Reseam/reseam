// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod dex;
pub mod documents;
pub mod manifest;
pub mod session;
pub mod table_bytes;
pub mod table_cases;
pub mod tables;
pub mod xml_cases;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;
