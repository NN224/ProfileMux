use crate::error::Result;
use crate::update::source::ReleaseAsset;

/// Stable release asset naming contract. Centralized here so the workflow, the
/// updater and the tests cannot drift apart.
pub fn asset_name_for_arch(arch: &str) -> Result<String> {
    unimplemented!("implemented by the updater task")
}

/// Checksum file published alongside a binary asset.
pub fn checksum_asset_name(binary_asset: &str) -> String {
    format!("{binary_asset}.sha256")
}

/// Picks exactly the asset for this machine. Ambiguity is an error, never a guess.
pub fn select_asset<'a>(assets: &'a [ReleaseAsset], arch: &str) -> Result<&'a ReleaseAsset> {
    unimplemented!("implemented by the updater task")
}
