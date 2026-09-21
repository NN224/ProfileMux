use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use semver::Version;
use serde::Deserialize;

use crate::error::{Error, Result};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB

fn ureq_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build()
}

fn user_agent() -> String {
    format!("profilemux/{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// One downloadable file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

/// A published, non-draft, non-prerelease release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub assets: Vec<ReleaseAsset>,
}

/// Where release metadata comes from. Implemented by the GitHub client in
/// production and by fixtures in tests, so no test needs the network.
pub trait ReleaseSource {
    fn latest_release(&self) -> Result<ReleaseInfo>;
    fn download(&self, asset: &ReleaseAsset) -> Result<Vec<u8>>;
}

/// Reads public release metadata from the GitHub REST API. No token required.
pub struct GithubReleaseSource {
    pub owner: String,
    pub repo: String,
}

impl GithubReleaseSource {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        GithubReleaseSource {
            owner: owner.into(),
            repo: repo.into(),
        }
    }
}

impl ReleaseSource for GithubReleaseSource {
    fn latest_release(&self) -> Result<ReleaseInfo> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.owner, self.repo
        );
        let resp = ureq_agent()
            .get(&url)
            .set("User-Agent", &user_agent())
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| {
                Error::Update(format!("failed to fetch latest release from GitHub: {e}"))
            })?;

        let body = resp
            .into_string()
            .map_err(|e| Error::Update(format!("failed to read response body: {e}")))?;

        let release: GithubRelease = serde_json::from_str(&body)
            .map_err(|e| Error::Update(format!("failed to parse GitHub release JSON: {e}")))?;

        if release.draft || release.prerelease {
            return Err(Error::Update(
                "no stable release was found (release is draft or prerelease)".to_string(),
            ));
        }

        let version = crate::update::parse_version(&release.tag_name)?;
        let assets = release
            .assets
            .into_iter()
            .map(|a| ReleaseAsset {
                name: a.name,
                download_url: a.browser_download_url,
                size: a.size,
            })
            .collect();

        Ok(ReleaseInfo {
            tag: release.tag_name,
            version,
            assets,
        })
    }

    fn download(&self, asset: &ReleaseAsset) -> Result<Vec<u8>> {
        if asset.size > MAX_DOWNLOAD_BYTES {
            return Err(Error::Update(format!(
                "asset size exceeds 64 MiB cap: {} bytes",
                asset.size
            )));
        }

        let resp = ureq_agent()
            .get(&asset.download_url)
            .set("User-Agent", &user_agent())
            .call()
            .map_err(|e| {
                Error::Update(format!("failed to download asset `{}`: {e}", asset.name))
            })?;

        let mut reader = resp.into_reader().take(MAX_DOWNLOAD_BYTES + 1);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(|e| {
            Error::Update(format!(
                "failed to read asset data for `{}`: {e}",
                asset.name
            ))
        })?;

        if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
            return Err(Error::Update(format!(
                "download of asset `{}` exceeded 64 MiB limit",
                asset.name
            )));
        }

        if asset.size != 0 && (bytes.len() as u64) != asset.size {
            return Err(Error::Update(format!(
                "downloaded asset `{}` size mismatch: expected {} bytes, got {} bytes",
                asset.name,
                asset.size,
                bytes.len()
            )));
        }

        Ok(bytes)
    }
}

/// Test double providing canned release data and asset payloads without network access.
#[derive(Debug, Clone)]
pub struct FixtureReleaseSource {
    pub release: ReleaseInfo,
    pub payloads: HashMap<String, Vec<u8>>,
    pub fail_with: Option<String>,
}

impl FixtureReleaseSource {
    pub fn new(release: ReleaseInfo) -> Self {
        Self {
            release,
            payloads: HashMap::new(),
            fail_with: None,
        }
    }
}

impl ReleaseSource for FixtureReleaseSource {
    fn latest_release(&self) -> Result<ReleaseInfo> {
        if let Some(err) = &self.fail_with {
            return Err(Error::Update(err.clone()));
        }
        Ok(self.release.clone())
    }

    fn download(&self, asset: &ReleaseAsset) -> Result<Vec<u8>> {
        if let Some(err) = &self.fail_with {
            return Err(Error::Update(err.clone()));
        }
        match self.payloads.get(&asset.name) {
            Some(payload) => Ok(payload.clone()),
            None => Err(Error::Update(format!(
                "no fixture payload for asset `{}`",
                asset.name
            ))),
        }
    }
}
