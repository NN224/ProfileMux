use crate::error::{Error, Result};
use crate::update::source::ReleaseAsset;

/// Stable release asset naming contract. Centralized here so the workflow, the
/// updater and the tests cannot drift apart.
pub fn asset_name_for_arch(arch: &str) -> Result<String> {
    match arch {
        "aarch64" => Ok("pmux-macos-aarch64".to_string()),
        "x86_64" => Ok("pmux-macos-x86_64".to_string()),
        other => Err(Error::Update(format!("unsupported architecture `{other}`"))),
    }
}

/// All binary asset names supported across architectures.
pub fn expected_asset_names() -> Vec<String> {
    vec![
        "pmux-macos-aarch64".to_string(),
        "pmux-macos-x86_64".to_string(),
    ]
}

/// Checksum file published alongside a binary asset.
pub fn checksum_asset_name(binary_asset: &str) -> String {
    format!("{binary_asset}.sha256")
}

/// Picks exactly the asset for this machine. Ambiguity is an error, never a guess.
pub fn select_asset<'a>(assets: &'a [ReleaseAsset], arch: &str) -> Result<&'a ReleaseAsset> {
    let expected = asset_name_for_arch(arch)?;
    let matches: Vec<&'a ReleaseAsset> = assets.iter().filter(|a| a.name == expected).collect();

    match matches.len() {
        0 => {
            let present = assets
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(Error::Update(format!(
                "expected asset `{expected}` not found; present: [{present}]"
            )))
        }
        1 => Ok(matches[0]),
        count => Err(Error::Update(format!(
            "multiple ({count}) assets found matching `{expected}`"
        ))),
    }
}
