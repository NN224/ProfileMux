use crate::error::Result;
use semver::Version;

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
        unimplemented!("implemented by the updater task")
    }

    fn download(&self, _asset: &ReleaseAsset) -> Result<Vec<u8>> {
        unimplemented!("implemented by the updater task")
    }
}
