use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::browsers::BrowserAdapter;
use crate::cli::output;
use crate::cli::selector;
use crate::domain::{
    BrowserInstall, BrowserProfile, ClonePolicy, CloneProfileSpec, CreateProfileSpec,
    ExtensionPolicy, HealthFinding,
};

#[derive(Parser, Debug)]
#[command(name = "pmux", about = "Cross-browser profile manager", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List discovered browser installations
    Browsers(BrowsersArgs),

    /// List profiles across browsers
    Profiles(ProfilesArgs),

    /// Inspect or act on a specific profile
    Profile {
        #[command(subcommand)]
        command: ProfileSubcommand,
    },

    /// Cache operations across browsers
    Cache {
        #[command(subcommand)]
        command: CacheSubcommand,
    },

    /// Diagnose browser installations and profile stores
    Doctor(DoctorArgs),
}

#[derive(Args, Debug)]
pub struct BrowsersArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ProfilesArgs {
    /// Filter profiles by browser slug
    #[arg(long)]
    pub browser: Option<String>,

    /// Measure disk usage for each profile
    #[arg(long)]
    pub sizes: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProfileSubcommand {
    /// Show full details for one profile
    Show(ProfileShowArgs),

    /// Open or reveal a profile directory
    Open(ProfileOpenArgs),

    /// Launch a profile in its browser
    Launch(ProfileLaunchArgs),

    /// Create a new profile
    Create(ProfileCreateArgs),

    /// Clone an existing profile
    Clone(ProfileCloneArgs),

    /// Rename a profile display name or directory
    Rename(ProfileRenameArgs),

    /// Set a profile avatar image
    Avatar(ProfileAvatarArgs),

    /// Delete a profile and move it to Trash
    Delete(ProfileDeleteArgs),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CacheSubcommand {
    /// Clean cache for a profile
    Clean(CacheCleanArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ProfileShowArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileOpenArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileLaunchArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileCreateArgs {
    /// Target browser slug
    #[arg(long)]
    pub browser: String,

    /// Profile display name
    #[arg(long)]
    pub name: String,

    /// Profile directory name
    #[arg(long)]
    pub directory: Option<String>,

    /// Template profile display name or directory within the browser
    #[arg(long)]
    pub template: Option<String>,

    /// Path to avatar image
    #[arg(long)]
    pub avatar: Option<PathBuf>,

    /// Extension copy policy (none, copy, copy-settings)
    #[arg(long, default_value = "none", value_parser = parse_extension_policy)]
    pub extensions: ExtensionPolicy,

    /// Open browser after profile creation
    #[arg(long)]
    pub open: bool,

    /// Simulate profile creation without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Close browser if running
    #[arg(long)]
    pub close_browser: bool,
}

impl ProfileCreateArgs {
    pub fn to_spec(&self) -> CreateProfileSpec {
        CreateProfileSpec {
            display_name: self.name.clone(),
            directory: self.directory.clone(),
            template_directory: self.template.clone(),
            avatar_source: self.avatar.clone(),
            clone_policy: ClonePolicy {
                copy_preferences: true,
                copy_bookmarks: false,
                extensions: self.extensions,
            },
            open_after_create: self.open,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct ProfileCloneArgs {
    /// Source profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// Profile display name
    #[arg(long)]
    pub name: String,

    /// Profile directory name
    #[arg(long)]
    pub directory: Option<String>,

    /// Path to avatar image
    #[arg(long)]
    pub avatar: Option<PathBuf>,

    /// Extension copy policy (none, copy, copy-settings)
    #[arg(long, default_value = "none", value_parser = parse_extension_policy)]
    pub extensions: ExtensionPolicy,

    /// Open browser after profile creation
    #[arg(long)]
    pub open: bool,

    /// Simulate profile clone without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Close browser if running
    #[arg(long)]
    pub close_browser: bool,
}

impl ProfileCloneArgs {
    pub fn to_spec(&self) -> CloneProfileSpec {
        CloneProfileSpec {
            display_name: self.name.clone(),
            directory: self.directory.clone(),
            avatar_source: self.avatar.clone(),
            clone_policy: ClonePolicy {
                copy_preferences: true,
                copy_bookmarks: false,
                extensions: self.extensions,
            },
            open_after_create: self.open,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct ProfileRenameArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// New display name
    #[arg(long, required_unless_present = "directory")]
    pub name: Option<String>,

    /// New directory name (experimental)
    #[arg(long, required_unless_present = "name")]
    pub directory: Option<String>,

    /// Simulate rename without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Close browser if running
    #[arg(long)]
    pub close_browser: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileAvatarArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// Path to avatar image
    pub image: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileDeleteArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// Simulate deletion without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Skip interactive confirmation
    #[arg(long)]
    pub yes: bool,

    /// Close browser if running
    #[arg(long)]
    pub close_browser: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CacheCleanArgs {
    /// Profile selector (ProfileId, slug/dir, or name)
    pub selector: String,

    /// Simulate cache clean without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Skip interactive confirmation
    #[arg(long)]
    pub yes: bool,

    /// Close browser if running
    #[arg(long)]
    pub close_browser: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Diagnose a single browser by slug
    #[arg(long)]
    pub browser: Option<String>,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightDecision {
    Proceed,
    RequestQuit,
    Refuse,
}

pub fn decide_browser_preflight(is_running: bool, close_browser: bool) -> PreflightDecision {
    if !is_running {
        PreflightDecision::Proceed
    } else if close_browser {
        PreflightDecision::RequestQuit
    } else {
        PreflightDecision::Refuse
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmDecision {
    Proceed,
    Prompt,
    Refuse,
}

pub fn decide_confirmation(yes: bool, is_terminal: bool) -> ConfirmDecision {
    if yes {
        ConfirmDecision::Proceed
    } else if !is_terminal {
        ConfirmDecision::Refuse
    } else {
        ConfirmDecision::Prompt
    }
}

pub fn parse_extension_policy(s: &str) -> std::result::Result<ExtensionPolicy, String> {
    ExtensionPolicy::from_cli(s).ok_or_else(|| {
        format!("invalid extension policy `{s}`, expected `none`, `copy`, or `copy-settings`")
    })
}

pub fn execute(command: Command) -> anyhow::Result<std::process::ExitCode> {
    match command {
        Command::Browsers(args) => run_browsers(args),
        Command::Profiles(args) => run_profiles(args),
        Command::Profile { command: sub } => match sub {
            ProfileSubcommand::Show(args) => run_profile_show(args),
            ProfileSubcommand::Open(args) => run_profile_open(args),
            ProfileSubcommand::Launch(args) => crate::cli::mutate::run_profile_launch(args),
            ProfileSubcommand::Create(args) => crate::cli::mutate::run_profile_create(args),
            ProfileSubcommand::Clone(args) => crate::cli::mutate::run_profile_clone(args),
            ProfileSubcommand::Rename(args) => crate::cli::mutate::run_profile_rename(args),
            ProfileSubcommand::Avatar(args) => crate::cli::mutate::run_profile_avatar(args),
            ProfileSubcommand::Delete(args) => crate::cli::mutate::run_profile_delete(args),
        },
        Command::Cache { command: sub } => match sub {
            CacheSubcommand::Clean(args) => crate::cli::mutate::run_cache_clean(args),
        },
        Command::Doctor(args) => run_doctor(args),
    }
}

pub fn run_browsers(args: BrowsersArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    if args.json {
        let installs: Vec<&BrowserInstall> = adapters.iter().map(|a| a.install()).collect();
        output::print_json(&installs)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let table = render_browsers_table(&adapters);
    table.print();
    Ok(std::process::ExitCode::SUCCESS)
}

fn render_browsers_table(adapters: &[Box<dyn BrowserAdapter>]) -> output::Table {
    let headers = vec![
        "NAME".to_string(),
        "CHANNEL".to_string(),
        "SLUG".to_string(),
        "VERSION".to_string(),
        "BUNDLE ID".to_string(),
        "USER DATA ROOT".to_string(),
        "PROFILES".to_string(),
        "SUPPORT".to_string(),
    ];
    let mut table = output::Table::new(headers);

    for adapter in adapters {
        let install = adapter.install();
        let profile_count = adapter.list_profiles().map(|p| p.len()).unwrap_or(0);
        table.add_row(vec![
            install.name.clone(),
            install.channel.label().to_string(),
            install.kind.slug().to_string(),
            install.version.as_deref().unwrap_or("-").to_string(),
            install.bundle_id.as_deref().unwrap_or("-").to_string(),
            install.user_data_root.display().to_string(),
            profile_count.to_string(),
            install.support.label().to_string(),
        ]);
    }
    table
}

pub fn run_profiles(args: ProfilesArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let filtered_adapters: Vec<_> = if let Some(ref slug) = args.browser {
        let matched: Vec<_> = adapters
            .into_iter()
            .filter(|a| a.install().kind.slug() == slug)
            .collect();
        if matched.is_empty() {
            return Err(crate::error::Error::NoSuchBrowser(slug.clone()).into());
        }
        matched
    } else {
        adapters
    };

    let mut profiles_with_install = Vec::new();
    for adapter in &filtered_adapters {
        let profiles = adapter.list_profiles()?;
        for profile in profiles {
            let measured = if args.sizes {
                let breakdown =
                    crate::fs::size::measure_profile(&profile.path, profile.cache_path.as_deref());
                profile.with_size(breakdown)
            } else {
                profile
            };
            profiles_with_install.push((adapter.install(), measured));
        }
    }

    if args.json {
        let profiles_only: Vec<&BrowserProfile> =
            profiles_with_install.iter().map(|(_, p)| p).collect();
        output::print_json(&profiles_only)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let table = render_profiles_table(&profiles_with_install, args.sizes);
    table.print();
    Ok(std::process::ExitCode::SUCCESS)
}

fn status_glyph(profile: &BrowserProfile) -> &'static str {
    if !profile.directory_exists {
        crate::domain::Severity::Broken.glyph()
    } else if !profile.registered {
        crate::domain::Severity::Warning.glyph()
    } else {
        crate::domain::Severity::Ok.glyph()
    }
}

fn render_profiles_table(
    profiles_with_install: &[(&BrowserInstall, BrowserProfile)],
    show_sizes: bool,
) -> output::Table {
    let mut headers = vec![
        "BROWSER".to_string(),
        "DISPLAY NAME".to_string(),
        "DIRECTORY".to_string(),
        "STATUS".to_string(),
    ];
    if show_sizes {
        headers.push("SIZE".to_string());
    }

    let mut table = output::Table::new(headers);
    for (install, profile) in profiles_with_install {
        let status = status_glyph(profile);
        let mut row = vec![
            install.kind.slug().to_string(),
            profile.display_name.clone(),
            profile.directory.clone(),
            status.to_string(),
        ];
        if show_sizes {
            let size_str = profile
                .size
                .map(|s| crate::fs::size::format_bytes(s.total))
                .unwrap_or_else(|| "-".to_string());
            row.push(size_str);
        }
        table.add_row(row);
    }
    table
}

#[derive(serde::Serialize)]
struct ProfileDetailJson<'a> {
    #[serde(flatten)]
    profile: &'a BrowserProfile,
    is_running: bool,
}

pub fn run_profile_show(args: ProfileShowArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (install, raw_profile) = selector::resolve_profile(&data, &args.selector)?;

    let is_running = adapters
        .iter()
        .find(|a| a.install().id == install.id)
        .map(|a| a.is_running())
        .unwrap_or(false);

    let breakdown =
        crate::fs::size::measure_profile(&raw_profile.path, raw_profile.cache_path.as_deref());
    let profile = raw_profile.with_size(breakdown);

    if args.json {
        let detail = ProfileDetailJson {
            profile: &profile,
            is_running,
        };
        output::print_json(&detail)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    print_profile_details(install, &profile, is_running);
    Ok(std::process::ExitCode::SUCCESS)
}

fn print_profile_details(install: &BrowserInstall, profile: &BrowserProfile, is_running: bool) {
    let avatar_str = match &profile.avatar {
        Some(av) => format!(
            "icon: {}, custom picture: {}",
            av.icon.as_deref().unwrap_or("none"),
            if av.uses_picture { "yes" } else { "no" }
        ),
        None => "none".to_string(),
    };
    let last_active_str = match profile.last_active {
        Some(ts) => ts.to_string(),
        None => "never".to_string(),
    };
    let cache_str = match &profile.cache_path {
        Some(p) => p.display().to_string(),
        None => "-".to_string(),
    };
    let breakdown = profile.size.unwrap_or_default();

    println!("Display Name:      {}", profile.display_name);
    println!("Directory:         {}", profile.directory);
    println!("Browser:           {}", install.label());
    println!("Absolute Path:     {}", profile.path.display());
    println!("Cache Path:        {cache_str}");
    println!("Avatar:            {avatar_str}");
    println!("Last Active:       {last_active_str}");
    println!(
        "Registered:        {}",
        if profile.registered { "yes" } else { "no" }
    );
    println!(
        "Directory Exists:  {}",
        if profile.directory_exists {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "Running:           {}",
        if is_running { "yes" } else { "no" }
    );
    println!("Sizes:");
    println!(
        "  Total:           {}",
        crate::fs::size::format_bytes(breakdown.total)
    );
    println!(
        "  Core:            {}",
        crate::fs::size::format_bytes(breakdown.core)
    );
    println!(
        "  Cache:           {}",
        crate::fs::size::format_bytes(breakdown.cache)
    );
    println!(
        "  Code Cache:      {}",
        crate::fs::size::format_bytes(breakdown.code_cache)
    );
    println!(
        "  GPU Cache:       {}",
        crate::fs::size::format_bytes(breakdown.gpu_cache)
    );
}

pub fn run_profile_open(args: ProfileOpenArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let data = selector::collect_installed_profiles(&adapters)?;
    let (_install, profile) = selector::resolve_profile(&data, &args.selector)?;

    println!("{}", profile.path.display());

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(&profile.path)
            .status();
    }

    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_doctor(args: DoctorArgs) -> anyhow::Result<std::process::ExitCode> {
    let adapters = crate::browsers::discover_all();
    let filtered_adapters: Vec<_> = if let Some(ref slug) = args.browser {
        let matched: Vec<_> = adapters
            .into_iter()
            .filter(|a| a.install().kind.slug() == slug)
            .collect();
        if matched.is_empty() {
            return Err(crate::error::Error::NoSuchBrowser(slug.clone()).into());
        }
        matched
    } else {
        adapters
    };

    let mut grouped = Vec::new();
    let mut all_findings = Vec::new();
    for adapter in &filtered_adapters {
        let findings = adapter.doctor()?;
        all_findings.extend(findings.clone());
        grouped.push((adapter.install(), findings));
    }

    if args.json {
        output::print_json(&all_findings)?;
    } else {
        for (install, findings) in &grouped {
            print_doctor_group(install, findings);
        }
        println!("{}", crate::doctor::summary_line(&all_findings));
    }

    let broken = crate::doctor::overall_severity(&all_findings) == crate::domain::Severity::Broken;
    Ok(if broken {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    })
}

fn print_doctor_group(install: &BrowserInstall, findings: &[HealthFinding]) {
    println!("{}", install.label());
    if findings.is_empty() {
        println!("  {} Healthy", crate::domain::Severity::Ok.glyph());
    } else {
        for finding in findings {
            println!(
                "  {} [{}] {}",
                finding.severity.glyph(),
                finding.code,
                finding.message
            );
            for path in &finding.paths {
                println!("      {}", path.display());
            }
        }
    }
}
