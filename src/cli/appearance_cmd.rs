use std::io::IsTerminal;
use std::path::Path;

use crate::browsers::BrowserAdapter;
use crate::cli::commands::{
    decide_browser_preflight, decide_confirmation, ConfirmDecision, PreflightDecision,
    ProfileAppearanceArgs,
};
use crate::cli::selector;
use crate::domain::{
    Appearance, AppearanceCapabilities, AppearanceSpec, BrowserInstallId, BrowserProfile,
    BrowserTheme, WebDarkMode,
};

/// Builds an `AppearanceSpec` from CLI option strings.
pub fn build_spec(theme: Option<&str>, web_dark: Option<&str>) -> anyhow::Result<AppearanceSpec> {
    let theme_val = match theme {
        Some(s) => Some(BrowserTheme::from_cli(s).ok_or_else(|| {
            anyhow::anyhow!("invalid theme `{s}`, expected `system`, `dark`, or `ultra-dark`")
        })?),
        None => None,
    };

    let web_dark_val = match web_dark {
        Some(s) => Some(WebDarkMode::from_cli(s).ok_or_else(|| {
            anyhow::anyhow!("invalid web dark mode `{s}`, expected `off` or `force`")
        })?),
        None => None,
    };

    Ok(AppearanceSpec {
        theme: theme_val,
        web_dark: web_dark_val,
    })
}

/// Renders the appearance inspection table matching `pmux profile show` column alignment.
pub fn render_inspect(name: &str, appearance: &Appearance) -> String {
    format!(
        "Profile:           {name}\nBrowser Theme:     {}\nWeb Dark Mode:     {}",
        appearance.theme.label(),
        appearance.web_dark.label()
    )
}

/// Validates whether the requested appearance changes are supported by the browser adapter.
pub fn check_supported(spec: &AppearanceSpec, caps: AppearanceCapabilities) -> anyhow::Result<()> {
    if spec.is_empty() {
        anyhow::bail!("no appearance changes specified");
    }

    if let Some(theme) = spec.theme {
        if theme == BrowserTheme::UltraDark {
            if !caps.ultra_dark {
                let reason = caps
                    .reason_unavailable("ultra-dark")
                    .unwrap_or("Brave-only");
                anyhow::bail!("Ultra Dark is unavailable: {reason}");
            }
        } else if !caps.browser_theme {
            let reason = caps
                .reason_unavailable("theme")
                .unwrap_or("not supported by this browser");
            anyhow::bail!("Browser theme is unavailable: {reason}");
        }
    }

    if spec.web_dark.is_some() && !caps.web_dark {
        let reason = caps
            .reason_unavailable("web-dark")
            .unwrap_or("not supported by this browser");
        anyhow::bail!("Web dark mode is unavailable: {reason}");
    }

    Ok(())
}

/// Renders the dry-run output for a web dark mode launch policy update.
pub fn render_web_dark_dry_run(path: &Path, from: WebDarkMode, to: WebDarkMode) -> String {
    format!(
        "Launch Policy:    {}\nWeb Dark Mode:    {} -> {}",
        path.display(),
        from.label(),
        to.label()
    )
}

fn find_adapter<'a>(
    adapters: &'a [Box<dyn BrowserAdapter>],
    install_id: &BrowserInstallId,
) -> anyhow::Result<&'a dyn BrowserAdapter> {
    adapters
        .iter()
        .find(|a| a.install().id == *install_id)
        .map(|a| a.as_ref())
        .ok_or_else(|| anyhow::anyhow!("adapter not found for installation"))
}

fn ensure_browser_closed(adapter: &dyn BrowserAdapter, close_browser: bool) -> anyhow::Result<()> {
    match decide_browser_preflight(adapter.is_running(), close_browser) {
        PreflightDecision::Proceed => Ok(()),
        PreflightDecision::Refuse => {
            let name = &adapter.install().name;
            anyhow::bail!(
                "{name} is currently running. Quit the browser or pass --close-browser to continue."
            );
        }
        PreflightDecision::RequestQuit => {
            adapter.request_quit()?;
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(15);
            while adapter.is_running() {
                if start.elapsed() >= timeout {
                    let name = &adapter.install().name;
                    anyhow::bail!(
                        "{name} is still running after request_quit. Please close it manually."
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Ok(())
        }
    }
}

fn prompt_confirmation(prompt: &str, yes: bool) -> anyhow::Result<bool> {
    match decide_confirmation(yes, std::io::stdin().is_terminal()) {
        ConfirmDecision::Proceed => Ok(true),
        ConfirmDecision::Refuse => {
            anyhow::bail!("confirmation required: pass --yes when stdin is not a terminal");
        }
        ConfirmDecision::Prompt => {
            use std::io::Write;
            print!("{prompt} [y/N]: ");
            std::io::stdout().flush()?;
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            Ok(line.trim().eq_ignore_ascii_case("y"))
        }
    }
}

fn run_dry_run(
    adapter: &dyn BrowserAdapter,
    profile: &BrowserProfile,
    spec: &AppearanceSpec,
) -> anyhow::Result<std::process::ExitCode> {
    if spec.theme.is_some() {
        let plan = adapter.plan_set_appearance(profile, spec)?;
        println!("{plan}");
    }
    if let Some(to) = spec.web_dark {
        let policy_path = crate::policy::default_path()?;
        let store = crate::policy::load(&policy_path)?;
        let from = store.get(&profile.id).web_dark;
        if spec.theme.is_some() {
            println!();
        }
        println!("{}", render_web_dark_dry_run(&policy_path, from, to));
    }
    println!("No files changed.");
    Ok(std::process::ExitCode::SUCCESS)
}

fn print_summary(profile: &BrowserProfile, spec: &AppearanceSpec) {
    match (spec.theme, spec.web_dark) {
        (Some(theme), Some(web_dark)) => {
            println!(
                "Updated appearance for profile '{}': Browser Theme: {}, Web Dark Mode: {}",
                profile.display_name,
                theme.label(),
                web_dark.label()
            );
        }
        (Some(theme), None) => {
            println!(
                "Updated appearance for profile '{}': Browser Theme: {}",
                profile.display_name,
                theme.label()
            );
        }
        (None, Some(web_dark)) => {
            println!(
                "Updated appearance for profile '{}': Web Dark Mode: {}",
                profile.display_name,
                web_dark.label()
            );
        }
        (None, None) => {}
    }
}

fn apply_appearance(
    adapter: &dyn BrowserAdapter,
    profile: &BrowserProfile,
    spec: &AppearanceSpec,
    close_browser: bool,
) -> anyhow::Result<()> {
    if spec.theme.is_some() {
        let _plan = adapter.plan_set_appearance(profile, spec)?;
        ensure_browser_closed(adapter, close_browser)?;
        adapter.set_appearance(profile, spec)?;
    }
    if let Some(web_dark) = spec.web_dark {
        let policy_path = crate::policy::default_path()?;
        let store = crate::policy::load(&policy_path)?;
        let store = store.set(&profile.id, crate::policy::LaunchPolicy { web_dark });
        store.save()?;
        println!("Web Dark Mode takes effect the next time ProfileMux launches this profile.");
    }
    print_summary(profile, spec);
    Ok(())
}

fn run_inspect(
    adapter: &dyn BrowserAdapter,
    profile: &BrowserProfile,
) -> anyhow::Result<std::process::ExitCode> {
    let mut appearance = adapter.read_appearance(profile)?;
    // A missing or unreadable policy file must not stop inspection.
    if let Ok(store) = crate::policy::default_path().and_then(|p| crate::policy::load(&p)) {
        appearance.web_dark = store.get(&profile.id).web_dark;
    }
    println!("{}", render_inspect(&profile.display_name, &appearance));
    Ok(std::process::ExitCode::SUCCESS)
}

/// Executes the profile appearance command.
pub fn run_profile_appearance(
    args: ProfileAppearanceArgs,
) -> anyhow::Result<std::process::ExitCode> {
    let spec = build_spec(args.theme.as_deref(), args.web_dark.as_deref())?;

    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    if spec.is_empty() {
        return run_inspect(adapter, profile);
    }

    check_supported(&spec, adapter.appearance_capabilities())?;

    if args.dry_run {
        return run_dry_run(adapter, profile, &spec);
    }

    let prompt = format!("Change appearance for profile '{}'?", profile.display_name);
    if !prompt_confirmation(&prompt, args.yes)? {
        println!("Aborted.");
        return Ok(std::process::ExitCode::FAILURE);
    }

    apply_appearance(adapter, profile, &spec, args.close_browser)?;
    Ok(std::process::ExitCode::SUCCESS)
}
