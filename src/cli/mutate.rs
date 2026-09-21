use std::io::IsTerminal;

use crate::browsers::BrowserAdapter;
use crate::cli::commands::{
    decide_browser_preflight, decide_confirmation, CacheCleanArgs, ConfirmDecision,
    PreflightDecision, ProfileAvatarArgs, ProfileCloneArgs, ProfileCreateArgs, ProfileDeleteArgs,
    ProfileLaunchArgs, ProfileRenameArgs,
};
use crate::cli::selector;
use crate::domain::{
    BrowserInstallId, BrowserProfile, DeleteMode, OperationKind, OperationPlan, PlanStep,
};
use crate::error::Error;

pub fn run_profile_launch(args: ProfileLaunchArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    check_capability(adapter, "launch")?;
    adapter.launch_profile(profile)?;
    println!(
        "Launched profile '{}' ({}).",
        profile.display_name,
        install.label()
    );
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_profile_create(args: ProfileCreateArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let adapter = find_adapter_by_slug(&adapters, &args.browser)?;

    check_capability(adapter, "create")?;
    if args.avatar.is_some() {
        check_capability(adapter, "custom_avatar")?;
    }

    let mut spec = args.to_spec();
    if let Some(ref template_str) = args.template {
        let profiles = adapter.list_profiles()?;
        let template_prof =
            selector::resolve_template_in_browser(adapter.install(), &profiles, template_str)?;
        spec.template_directory = Some(template_prof.directory.clone());
    }

    let plan = adapter.plan_create(&spec)?;
    if args.dry_run {
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, args.close_browser)?;
    }

    let new_profile = adapter.create_profile(&spec)?;
    println!("Created profile '{}'", new_profile.display_name);
    println!("Directory:     {}", new_profile.directory);
    println!("Path:          {}", new_profile.path.display());
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_profile_clone(args: ProfileCloneArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, source_profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    check_capability(adapter, "clone")?;
    if args.avatar.is_some() {
        check_capability(adapter, "custom_avatar")?;
    }

    let spec = args.to_spec();
    let plan = adapter.plan_clone(source_profile, &spec)?;
    if args.dry_run {
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, args.close_browser)?;
    }

    let new_profile = adapter.clone_profile(source_profile, &spec)?;
    println!(
        "Cloned profile '{}' -> '{}'",
        source_profile.display_name, new_profile.display_name
    );
    println!("Directory:     {}", new_profile.directory);
    println!("Path:          {}", new_profile.path.display());
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_profile_rename(args: ProfileRenameArgs) -> anyhow::Result<std::process::ExitCode> {
    if args.name.is_none() && args.directory.is_none() {
        anyhow::bail!("at least one of --name or --directory must be specified");
    }

    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    if let Some(ref dir) = args.directory {
        run_rename_with_directory(adapter, profile, args.name.as_deref(), dir, &args)
    } else if let Some(ref name) = args.name {
        run_rename_display_name_only(adapter, profile, name, args.dry_run)
    } else {
        Ok(std::process::ExitCode::SUCCESS)
    }
}

fn run_rename_with_directory(
    adapter: &dyn BrowserAdapter,
    profile: &BrowserProfile,
    name: Option<&str>,
    dir: &str,
    args: &ProfileRenameArgs,
) -> anyhow::Result<std::process::ExitCode> {
    if name.is_some() {
        check_capability(adapter, "rename_display_name")?;
    }
    check_capability(adapter, "rename_directory")?;

    let mut plan = adapter.plan_rename_directory(profile, dir)?;
    if let Some(n) = name {
        plan.steps
            .insert(0, PlanStep::new(format!("Change display name to `{n}`")));
    }
    if args.dry_run {
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if let Some(n) = name {
        adapter.rename_display_name(profile, n)?;
    }
    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, args.close_browser)?;
    }
    adapter.rename_profile_directory(profile, dir)?;

    if let Some(n) = name {
        println!(
            "Renamed profile '{}': display name to '{}', directory to '{}'",
            profile.display_name, n, dir
        );
    } else {
        println!(
            "Renamed profile '{}' directory to '{}'",
            profile.display_name, dir
        );
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn run_rename_display_name_only(
    adapter: &dyn BrowserAdapter,
    profile: &BrowserProfile,
    new_name: &str,
    dry_run: bool,
) -> anyhow::Result<std::process::ExitCode> {
    check_capability(adapter, "rename_display_name")?;
    if dry_run {
        let mut plan = OperationPlan::new(
            OperationKind::RenameDisplayName,
            adapter.install().label(),
            &profile.directory,
        );
        plan.steps.push(PlanStep::new(format!(
            "Change display name to `{new_name}`"
        )));
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }
    adapter.rename_display_name(profile, new_name)?;
    println!(
        "Renamed profile '{}' display name to '{}'",
        profile.display_name, new_name
    );
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_profile_avatar(args: ProfileAvatarArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    check_capability(adapter, "custom_avatar")?;
    if !args.image.is_file() {
        return Err(Error::NotFound(args.image).into());
    }

    let plan = adapter.plan_set_avatar(profile, &args.image)?;
    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, false)?;
    }

    adapter.set_avatar(profile, &args.image)?;
    println!(
        "Updated avatar for profile '{}' from '{}'",
        profile.display_name,
        args.image.display()
    );
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_profile_delete(args: ProfileDeleteArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    check_capability(adapter, "delete")?;
    let plan = adapter.plan_delete(profile, DeleteMode::Trash)?;

    if args.dry_run {
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if !args.yes {
        println!("{plan}");
    }
    let prompt_msg = format!("Delete profile '{}'?", profile.display_name);
    if !prompt_confirmation(&prompt_msg, args.yes)? {
        println!("Aborted.");
        return Ok(std::process::ExitCode::FAILURE);
    }

    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, args.close_browser)?;
    }

    adapter.delete_profile(profile, DeleteMode::Trash)?;
    println!(
        "Deleted profile '{}' (moved to Trash)",
        profile.display_name
    );
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_cache_clean(args: CacheCleanArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, profile) = selector::resolve_profile(&data, &args.selector)?;
    let adapter = find_adapter(&adapters, &install.id)?;

    check_capability(adapter, "clean_cache")?;
    let plan = adapter.plan_clean_cache(profile)?;

    if args.dry_run {
        print_dry_run(&plan);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    if !args.yes {
        println!("{plan}");
    }
    let prompt_msg = format!("Clean cache for profile '{}'?", profile.display_name);
    if !prompt_confirmation(&prompt_msg, args.yes)? {
        println!("Aborted.");
        return Ok(std::process::ExitCode::FAILURE);
    }

    if plan.requires_browser_closed {
        ensure_browser_closed(adapter, args.close_browser)?;
    }

    if let Some(bytes) = plan.reclaimed_bytes {
        println!(
            "Estimated reclaimable: {}",
            crate::fs::size::format_bytes(bytes)
        );
    } else {
        println!("Estimated reclaimable: unknown");
    }

    let reclaimed = adapter.clean_cache(profile)?;
    println!(
        "Reclaimed:             {}",
        crate::fs::size::format_bytes(reclaimed)
    );
    Ok(std::process::ExitCode::SUCCESS)
}

fn print_dry_run(plan: &OperationPlan) {
    println!("{plan}");
    println!("No files changed.");
}

fn check_capability(adapter: &dyn BrowserAdapter, action: &str) -> anyhow::Result<()> {
    if let Some(reason) = adapter.capabilities().reason_disabled(action) {
        anyhow::bail!("{reason}");
    }
    Ok(())
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

fn find_adapter_by_slug<'a>(
    adapters: &'a [Box<dyn BrowserAdapter>],
    slug: &str,
) -> anyhow::Result<&'a dyn BrowserAdapter> {
    adapters
        .iter()
        .find(|a| a.install().kind.slug() == slug)
        .map(|a| a.as_ref())
        .ok_or_else(|| Error::NoSuchBrowser(slug.to_string()).into())
}
