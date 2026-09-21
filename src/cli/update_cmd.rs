use std::io::{IsTerminal, Write};
use std::path::Path;

use semver::Version;

use crate::cli::commands::UpdateArgs;
use crate::error::{Error, Result};
use crate::update::asset::{checksum_asset_name, select_asset};
use crate::update::install::{classify_install, install_binary, InstallKind};
use crate::update::source::{GithubReleaseSource, ReleaseSource};
use crate::update::verify::verify_sha256;
use crate::update::{compare, UpdateStatus, RELEASE_OWNER, RELEASE_REPO};

/// Decision outcome for update confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    Proceed,
    Cancel,
    RefuseNonInteractive,
}

/// Decides whether to proceed based on user input and terminal status.
pub fn decide_confirmation(answer: &str, yes_flag: bool, stdin_is_tty: bool) -> Confirmation {
    if yes_flag {
        return Confirmation::Proceed;
    }
    if !stdin_is_tty {
        return Confirmation::RefuseNonInteractive;
    }
    let trimmed = answer.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("y")
        || trimmed.eq_ignore_ascii_case("yes")
    {
        Confirmation::Proceed
    } else {
        Confirmation::Cancel
    }
}

/// Renders the output shape for `pmux update --check`.
pub fn render_check(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::UpToDate { current } => {
            format!("ProfileMux {current}\nAlready up to date.")
        }
        UpdateStatus::Available { current, latest } => {
            format!("ProfileMux {current}\nLatest: {latest}\n\nUpdate available.")
        }
    }
}

/// Formats the interactive confirmation question.
pub fn confirm_prompt(current: &Version, latest: &Version) -> String {
    format!("Update {current} -> {latest}? [Y/n]")
}

/// Executes the core update workflow: check, download, verify and install.
pub fn run_update(
    source: &dyn ReleaseSource,
    exe: &Path,
    arch: &str,
    current: &Version,
) -> Result<Option<Version>> {
    let install_kind = classify_install(exe);
    if install_kind == InstallKind::Development {
        return Err(Error::Update(
            "This appears to be a development build.".to_string(),
        ));
    }

    let release = source.latest_release()?;
    let status = compare(current, &release.version);
    let latest = match status {
        UpdateStatus::UpToDate { .. } => return Ok(None),
        UpdateStatus::Available { latest, .. } => latest,
    };

    if install_kind == InstallKind::CargoBin {
        println!("Note: a future `cargo install` can overwrite the updated binary.");
    }

    let binary_asset = select_asset(&release.assets, arch)?;
    let checksum_name = checksum_asset_name(&binary_asset.name);
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .ok_or_else(|| {
            Error::Update(format!(
                "missing checksum asset `{checksum_name}` in release"
            ))
        })?;

    println!("Downloading...");
    let binary_bytes = source.download(binary_asset)?;
    let checksum_bytes = source.download(checksum_asset)?;
    let checksum_str = std::str::from_utf8(&checksum_bytes)
        .map_err(|e| Error::Update(format!("invalid checksum utf-8: {e}")))?;

    println!("Verifying checksum...");
    verify_sha256(&binary_bytes, checksum_str)?;

    println!("Installing...");
    install_binary(&binary_bytes, exe)?;

    Ok(Some(latest))
}

fn prompt_user(
    current: &Version,
    latest: &Version,
    yes: bool,
    is_terminal: bool,
) -> anyhow::Result<Confirmation> {
    if !yes && !is_terminal {
        return Ok(Confirmation::RefuseNonInteractive);
    }

    println!("ProfileMux {current}");
    println!();
    println!("New version available: {latest}");

    if yes {
        return Ok(Confirmation::Proceed);
    }

    println!();
    print!("{} ", confirm_prompt(current, latest));
    std::io::stdout().flush()?;

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(decide_confirmation(&answer, false, true))
}

/// Entry point for `pmux update`.
pub fn run_update_cmd(args: UpdateArgs) -> anyhow::Result<std::process::ExitCode> {
    let current = crate::update::current_version()?;
    let source = GithubReleaseSource::new(RELEASE_OWNER, RELEASE_REPO);

    if args.check {
        let status = crate::update::check_update(&source, &current)?;
        println!("{}", render_check(&status));
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let status = crate::update::check_update(&source, &current)?;
    match status {
        UpdateStatus::UpToDate { current } => {
            println!("ProfileMux {current} is already up to date.");
            Ok(std::process::ExitCode::SUCCESS)
        }
        UpdateStatus::Available { current, latest } => {
            let is_terminal = std::io::stdin().is_terminal();
            let decision = prompt_user(&current, &latest, args.yes, is_terminal)?;
            match decision {
                Confirmation::RefuseNonInteractive => Err(Error::Update(
                    "confirmation required: pass --yes when stdin is not a terminal".to_string(),
                )
                .into()),
                Confirmation::Cancel => {
                    println!("Update cancelled.");
                    Ok(std::process::ExitCode::SUCCESS)
                }
                Confirmation::Proceed => {
                    let exe = std::env::current_exe().map_err(|e| {
                        Error::Update(format!("cannot determine current executable: {e}"))
                    })?;
                    let arch = std::env::consts::ARCH;
                    let outcome = run_update(&source, &exe, arch, &current)?;
                    if let Some(new_version) = outcome {
                        println!();
                        println!("Updated successfully to {new_version}.");
                        println!("Restart ProfileMux to use the new version.");
                    }
                    Ok(std::process::ExitCode::SUCCESS)
                }
            }
        }
    }
}
