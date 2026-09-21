use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use profilemux::cli::commands::{Cli, Command};
use profilemux::cli::update_cmd::{
    confirm_prompt, decide_confirmation, render_check, run_update, Confirmation,
};
use profilemux::update::source::{FixtureReleaseSource, ReleaseAsset, ReleaseInfo, ReleaseSource};
use profilemux::update::verify::sha256_hex;
use profilemux::update::UpdateStatus;
use semver::Version;

struct CountingReleaseSource {
    inner: FixtureReleaseSource,
    downloads: AtomicUsize,
}

impl CountingReleaseSource {
    fn new(inner: FixtureReleaseSource) -> Self {
        Self {
            inner,
            downloads: AtomicUsize::new(0),
        }
    }

    fn download_count(&self) -> usize {
        self.downloads.load(Ordering::SeqCst)
    }
}

impl ReleaseSource for CountingReleaseSource {
    fn latest_release(&self) -> profilemux::error::Result<ReleaseInfo> {
        self.inner.latest_release()
    }

    fn download(&self, asset: &ReleaseAsset) -> profilemux::error::Result<Vec<u8>> {
        self.downloads.fetch_add(1, Ordering::SeqCst);
        self.inner.download(asset)
    }
}

#[test]
fn cli_parse_update_flags() {
    let cli = Cli::try_parse_from(["pmux", "update"]).unwrap();
    match cli.command {
        Some(Command::Update(args)) => {
            assert!(!args.check);
            assert!(!args.yes);
        }
        _ => panic!("expected Command::Update"),
    }

    let cli_check = Cli::try_parse_from(["pmux", "update", "--check"]).unwrap();
    match cli_check.command {
        Some(Command::Update(args)) => {
            assert!(args.check);
            assert!(!args.yes);
        }
        _ => panic!("expected Command::Update"),
    }

    let cli_yes = Cli::try_parse_from(["pmux", "update", "--yes"]).unwrap();
    match cli_yes.command {
        Some(Command::Update(args)) => {
            assert!(!args.check);
            assert!(args.yes);
        }
        _ => panic!("expected Command::Update"),
    }

    assert!(Cli::try_parse_from(["pmux", "update", "--unknown-flag"]).is_err());
}

#[test]
fn render_check_up_to_date_output() {
    let current = Version::parse("1.1.0").unwrap();
    let status = UpdateStatus::UpToDate { current };
    let output = render_check(&status);
    assert!(output.contains("Already up to date."));
    assert!(!output.contains("Latest:"));
    assert!(output.contains("1.1.0"));
}

#[test]
fn render_check_available_output() {
    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();
    let status = UpdateStatus::Available { current, latest };
    let output = render_check(&status);
    assert!(output.contains("1.1.0"));
    assert!(output.contains("1.2.0"));
    assert!(output.contains("Update available."));
}

#[test]
fn confirm_prompt_formatting() {
    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();
    assert_eq!(
        confirm_prompt(&current, &latest),
        "Update 1.1.0 -> 1.2.0? [Y/n]"
    );
}

#[test]
fn decide_confirmation_rules() {
    assert_eq!(decide_confirmation("", false, true), Confirmation::Proceed);
    assert_eq!(
        decide_confirmation("   \n", false, true),
        Confirmation::Proceed
    );

    assert_eq!(decide_confirmation("y", false, true), Confirmation::Proceed);
    assert_eq!(
        decide_confirmation("YES", false, true),
        Confirmation::Proceed
    );
    assert_eq!(
        decide_confirmation("yes", false, true),
        Confirmation::Proceed
    );

    assert_eq!(decide_confirmation("n", false, true), Confirmation::Cancel);
    assert_eq!(decide_confirmation("N", false, true), Confirmation::Cancel);

    assert_eq!(decide_confirmation("no", false, true), Confirmation::Cancel);
    assert_eq!(
        decide_confirmation("random", false, true),
        Confirmation::Cancel
    );

    assert_eq!(decide_confirmation("n", true, false), Confirmation::Proceed);
    assert_eq!(
        decide_confirmation("anything", true, true),
        Confirmation::Proceed
    );

    assert_eq!(
        decide_confirmation("", false, false),
        Confirmation::RefuseNonInteractive
    );
    assert_eq!(
        decide_confirmation("y", false, false),
        Confirmation::RefuseNonInteractive
    );
}

#[test]
fn run_update_equal_version_performs_no_download() {
    let temp = tempfile::tempdir().unwrap();
    let fake_exe = temp.path().join("pmux");
    std::fs::write(&fake_exe, b"original-binary").unwrap();

    let current = Version::parse("1.1.0").unwrap();
    let release = ReleaseInfo {
        tag: "v1.1.0".to_string(),
        version: current.clone(),
        assets: vec![],
    };
    let fixture = FixtureReleaseSource::new(release);
    let counting = CountingReleaseSource::new(fixture);

    let result = run_update(&counting, &fake_exe, "aarch64", &current).unwrap();
    assert_eq!(result, None);
    assert_eq!(counting.download_count(), 0);
    assert_eq!(std::fs::read(&fake_exe).unwrap(), b"original-binary");
}

#[test]
fn run_update_replaces_executable_with_correct_checksum() {
    let temp = tempfile::tempdir().unwrap();
    let fake_exe = temp.path().join("pmux");
    std::fs::write(&fake_exe, b"old-binary-content").unwrap();

    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();

    let new_binary = b"new-binary-content-1.2.0";
    let sha = sha256_hex(new_binary);
    let checksum_content = format!("{sha}  pmux-macos-aarch64\n");

    let bin_asset = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/bin".to_string(),
        size: new_binary.len() as u64,
    };
    let chk_asset = ReleaseAsset {
        name: "pmux-macos-aarch64.sha256".to_string(),
        download_url: "https://example.com/sha".to_string(),
        size: checksum_content.len() as u64,
    };

    let release = ReleaseInfo {
        tag: "v1.2.0".to_string(),
        version: latest.clone(),
        assets: vec![bin_asset.clone(), chk_asset.clone()],
    };

    let mut fixture = FixtureReleaseSource::new(release);
    fixture.payloads.insert(bin_asset.name, new_binary.to_vec());
    fixture
        .payloads
        .insert(chk_asset.name, checksum_content.into_bytes());

    let result = run_update(&fixture, &fake_exe, "aarch64", &current).unwrap();
    assert_eq!(result, Some(latest));
    assert_eq!(std::fs::read(&fake_exe).unwrap(), new_binary);
}

#[test]
fn run_update_wrong_checksum_aborts_and_preserves_executable() {
    let temp = tempfile::tempdir().unwrap();
    let fake_exe = temp.path().join("pmux");
    let original_content = b"original-fake-executable";
    std::fs::write(&fake_exe, original_content).unwrap();

    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();

    let new_binary = b"tampered-binary-content";
    let bad_checksum = "0000000000000000000000000000000000000000000000000000000000000000\n";

    let bin_asset = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/bin".to_string(),
        size: new_binary.len() as u64,
    };
    let chk_asset = ReleaseAsset {
        name: "pmux-macos-aarch64.sha256".to_string(),
        download_url: "https://example.com/sha".to_string(),
        size: bad_checksum.len() as u64,
    };

    let release = ReleaseInfo {
        tag: "v1.2.0".to_string(),
        version: latest,
        assets: vec![bin_asset.clone(), chk_asset.clone()],
    };

    let mut fixture = FixtureReleaseSource::new(release);
    fixture.payloads.insert(bin_asset.name, new_binary.to_vec());
    fixture
        .payloads
        .insert(chk_asset.name, bad_checksum.as_bytes().to_vec());

    let result = run_update(&fixture, &fake_exe, "aarch64", &current);
    assert!(result.is_err());
    assert_eq!(std::fs::read(&fake_exe).unwrap(), original_content);
}

#[test]
fn run_update_refuses_development_build_and_performs_no_download() {
    let temp = tempfile::tempdir().unwrap();
    let dev_dir = temp.path().join("target").join("debug");
    std::fs::create_dir_all(&dev_dir).unwrap();
    let dev_exe = dev_dir.join("pmux");
    std::fs::write(&dev_exe, b"dev-binary").unwrap();

    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();

    let bin_asset = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/bin".to_string(),
        size: 100,
    };
    let chk_asset = ReleaseAsset {
        name: "pmux-macos-aarch64.sha256".to_string(),
        download_url: "https://example.com/sha".to_string(),
        size: 64,
    };

    let release = ReleaseInfo {
        tag: "v1.2.0".to_string(),
        version: latest,
        assets: vec![bin_asset, chk_asset],
    };

    let fixture = FixtureReleaseSource::new(release);
    let counting = CountingReleaseSource::new(fixture);

    let result = run_update(&counting, &dev_exe, "aarch64", &current);
    assert!(result.is_err());
    assert_eq!(counting.download_count(), 0);
    assert_eq!(std::fs::read(&dev_exe).unwrap(), b"dev-binary");
}
