// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use rusty_s3::actions::{ListObjectsV2, ListObjectsV2Response};
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use ureq::Agent;
use url::Url;

use super::env;
use super::http::{agent, checked, text_if_found};
use super::store::{Caching, Storage};

const SIGNATURE_LIFETIME: Duration = Duration::from_secs(600);

/// A prefix in an S3 bucket.
pub(super) struct S3 {
    bucket: Bucket,
    prefix: String,
    credentials: Credentials,
    agent: Agent,
}

impl S3 {
    pub fn from_env(bucket: &str, prefix: &str) -> Result<Self> {
        let env = |name| env(name, "S3 targets read the AWS_* variables");
        let endpoint = Url::parse(&env("AWS_ENDPOINT_URL")?).context("invalid AWS_ENDPOINT_URL")?;
        Ok(Self {
            bucket: Bucket::new(
                endpoint,
                UrlStyle::Path,
                bucket.to_owned(),
                env("AWS_REGION")?,
            )
            .context("invalid S3 bucket")?,
            prefix: prefix.to_owned(),
            credentials: Credentials::new(env("AWS_ACCESS_KEY_ID")?, env("AWS_SECRET_ACCESS_KEY")?),
            agent: agent(),
        })
    }

    /// The object key of `key`, a key relative to the prefix.
    fn object(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{}/{key}", self.prefix)
        }
    }

    fn list(
        &self,
        prefix: &str,
        delimiter: Option<&str>,
        mut page: impl FnMut(ListObjectsV2Response),
    ) -> Result<()> {
        let mut token: Option<String> = None;
        loop {
            let mut action: ListObjectsV2<'_> =
                self.bucket.list_objects_v2(Some(&self.credentials));
            action.with_prefix(prefix);
            if let Some(delimiter) = delimiter {
                action.with_delimiter(delimiter);
            }
            if let Some(token) = &token {
                action.with_continuation_token(token.clone());
            }
            let url = action.sign(SIGNATURE_LIFETIME);
            let text = checked(self.agent.get(url.as_str()).call())?
                .into_body()
                .read_to_string()?;
            let mut response =
                ListObjectsV2::parse_response(&text).context("S3 returned an invalid listing")?;
            token = response.next_continuation_token.take();
            page(response);
            if token.is_none() {
                return Ok(());
            }
        }
    }
}

impl fmt::Display for S3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s3://{}/{}", self.bucket.name(), self.prefix)
    }
}

impl Storage for S3 {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let url = self
            .bucket
            .get_object(Some(&self.credentials), &self.object(key))
            .sign(SIGNATURE_LIFETIME);
        text_if_found(self.agent.get(url.as_str()).call())
    }

    fn put(&self, key: &str, path: &Path, caching: Caching) -> Result<()> {
        let url = self
            .bucket
            .put_object(Some(&self.credentials), &self.object(key))
            .sign(SIGNATURE_LIFETIME);
        let file =
            File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        checked(
            self.agent
                .put(url.as_str())
                .header("Cache-Control", cache_control(caching))
                .header("Content-Type", content_type(path))
                .send(file),
        )?;
        Ok(())
    }

    fn folders(&self) -> Result<Vec<String>> {
        let prefix = self.object("");
        let mut folders = Vec::new();
        self.list(&prefix, Some("/"), |page| {
            folders.extend(page.common_prefixes.into_iter().filter_map(|common| {
                common
                    .prefix
                    .strip_prefix(&prefix)
                    .map(|name| name.trim_end_matches('/').to_owned())
            }));
        })?;
        Ok(folders)
    }

    fn delete_folder(&self, name: &str) -> Result<()> {
        let mut keys = Vec::new();
        self.list(&self.object(&format!("{name}/")), None, |page| {
            keys.extend(page.contents.into_iter().map(|object| object.key));
        })?;
        for key in keys {
            let url = self
                .bucket
                .delete_object(Some(&self.credentials), &key)
                .sign(SIGNATURE_LIFETIME);
            checked(self.agent.delete(url.as_str()).call())?;
        }
        Ok(())
    }

    fn public_path(&self, key: &str) -> String {
        self.object(key)
    }
}

fn cache_control(caching: Caching) -> &'static str {
    match caching {
        Caching::Release => "public, max-age=86400",
        Caching::Latest => "no-cache",
    }
}

fn content_type(path: &Path) -> &'static str {
    if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        "application/json"
    } else {
        "application/octet-stream"
    }
}
