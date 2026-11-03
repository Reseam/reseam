// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Planning a selection and reading option text, before any patch runs.

use boltffi::export;
use reseam_model::{
    ChangesRequest, OptionDeclaration, OptionProblem, OptionValue, PatchChange, PlanRequest,
    SdkError, SelectionPlan,
};

/// What a run of the request's selection would do for its target app, from
/// patch metadata alone. Selection entries that name no patch are listed
/// instead of failing.
#[export]
pub fn plan(request: PlanRequest) -> Result<SelectionPlan, SdkError> {
    reseam_sdk::plan(&request).map_err(SdkError::from)
}

/// How patching the app of the request's build again, with the patches
/// available now, would differ from that build; empty when it would not.
#[export]
pub fn changes(request: ChangesRequest) -> Result<Vec<PatchChange>, SdkError> {
    reseam_sdk::changes(&request).map_err(SdkError::from)
}

/// Reads `text` as a value of `option`, as a user types it: `true` or
/// `false`, a number, one list item per line, or the text itself.
#[export]
pub fn parse_option(option: OptionDeclaration, text: String) -> Result<OptionValue, OptionProblem> {
    option.parse(&text)
}

/// The text form of `value`, which `parseOption` reads back as the same value.
#[export]
pub fn option_text(value: OptionValue) -> String {
    value.to_string()
}
