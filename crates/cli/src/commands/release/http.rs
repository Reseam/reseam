// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use ureq::http::{Response, StatusCode};
use ureq::{Agent, Body, ResponseExt};

pub(crate) fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

pub(crate) fn checked(response: Result<Response<Body>, ureq::Error>) -> Result<Response<Body>> {
    let mut response = response?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let uri = response.get_uri().clone();
    let body = response.body_mut().read_to_string().unwrap_or_default();
    bail!("{uri} returned {status}: {}", body.trim())
}

pub(crate) fn json<T: DeserializeOwned>(
    response: Result<Response<Body>, ureq::Error>,
) -> Result<T> {
    let mut response = checked(response)?;
    let uri = response.get_uri().clone();
    response
        .body_mut()
        .read_json()
        .with_context(|| format!("{uri} returned unexpected JSON"))
}

pub(crate) fn text_if_found(
    response: Result<Response<Body>, ureq::Error>,
) -> Result<Option<String>> {
    let response = response?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(checked(Ok(response))?.into_body().read_to_string()?))
}
