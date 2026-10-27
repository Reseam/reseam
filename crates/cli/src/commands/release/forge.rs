// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tracing::info;
use ureq::Agent;
use url::Url;

use super::http::{agent, checked, json, text_if_found};
use super::{env, file_name};

pub(super) struct Forge {
    api: Url,
    page: Url,
    repository: String,
    commit: Option<String>,
    auth: String,
    agent: Agent,
}

#[derive(Deserialize)]
struct Release {
    id: u64,
    tag_name: String,
    draft: bool,
    upload_url: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

impl Forge {
    pub fn from_env() -> Result<Self> {
        let env = |name| env(name, "run in a GitHub or Forgejo workflow");
        let repository = env("GITHUB_REPOSITORY")?;
        let page = with_segments(
            Url::parse(&env("GITHUB_SERVER_URL")?).context("invalid GITHUB_SERVER_URL")?,
            repository.split('/'),
        );
        Ok(Self {
            api: Url::parse(&env("GITHUB_API_URL")?).context("invalid GITHUB_API_URL")?,
            page,
            repository,
            commit: std::env::var("GITHUB_SHA").ok(),
            auth: format!("token {}", env("RELEASE_TOKEN")?),
            agent: agent(),
        })
    }

    pub fn page(&self) -> &Url {
        &self.page
    }

    pub fn asset_url(&self, tag: &str, name: &str) -> Url {
        with_segments(self.page.clone(), ["releases", "download", tag, name])
    }

    pub fn latest_asset(&self, name: &str) -> Result<Option<String>> {
        let Some(release) = self.releases()?.into_iter().find(|release| !release.draft) else {
            return Ok(None);
        };
        let Some(asset) = release.assets.into_iter().find(|asset| asset.name == name) else {
            return Ok(None);
        };
        text_if_found(
            self.agent
                .get(&asset.browser_download_url)
                .header("Authorization", &self.auth)
                .call(),
        )
    }

    pub fn publish(&self, tag: &str, notes: &str, prerelease: bool, files: &[&Path]) -> Result<()> {
        for stale in self
            .releases()?
            .into_iter()
            .filter(|release| release.tag_name == tag)
        {
            checked(
                self.agent
                    .delete(self.release_url(Some(stale.id)).as_str())
                    .header("Authorization", &self.auth)
                    .call(),
            )?;
        }
        // A draft stays hidden from clients until every file is attached.
        let mut body = json!({
            "tag_name": tag,
            "name": tag,
            "body": notes,
            "prerelease": prerelease,
            "draft": true,
        });
        if let Some(commit) = &self.commit {
            body["target_commitish"] = json!(commit);
        }
        let release: Release = json(
            self.agent
                .post(self.release_url(None).as_str())
                .header("Authorization", &self.auth)
                .send_json(&body),
        )?;
        // GitHub ends upload_url with a {?name,label} URI template; Forgejo does not.
        let uploads = release
            .upload_url
            .split_once('{')
            .map_or(release.upload_url.as_str(), |(url, _)| url);
        for path in files {
            let mut url =
                Url::parse(uploads).context("the forge returned an invalid upload_url")?;
            url.query_pairs_mut().append_pair("name", file_name(path)?);
            let file =
                File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
            checked(
                self.agent
                    .post(url.as_str())
                    .header("Authorization", &self.auth)
                    .header("Content-Type", "application/octet-stream")
                    .send(file),
            )?;
        }
        checked(
            self.agent
                .patch(self.release_url(Some(release.id)).as_str())
                .header("Authorization", &self.auth)
                .send_json(json!({ "draft": false })),
        )?;
        info!(tag, page = %self.page, "published the forge release");
        Ok(())
    }

    fn releases(&self) -> Result<Vec<Release>> {
        json(
            self.agent
                .get(self.release_url(None).as_str())
                .header("Authorization", &self.auth)
                .call(),
        )
    }

    fn release_url(&self, id: Option<u64>) -> Url {
        let id = id.map(|id| id.to_string());
        with_segments(
            self.api.clone(),
            ["repos"]
                .into_iter()
                .chain(self.repository.split('/'))
                .chain(["releases"])
                .chain(id.as_deref()),
        )
    }
}

fn with_segments<'a>(mut url: Url, segments: impl IntoIterator<Item = &'a str>) -> Url {
    url.path_segments_mut()
        .expect("http URLs have path segments")
        .pop_if_empty()
        .extend(segments);
    url
}
