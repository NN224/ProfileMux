use std::path::{Path, PathBuf};

use clap::Parser;
use profilemux::cli::commands::{
    decide_browser_preflight, decide_confirmation, parse_extension_policy, CacheSubcommand, Cli,
    Command, ConfirmDecision, PreflightDecision, ProfileSubcommand,
};
use profilemux::cli::selector::resolve_template_in_browser;
use profilemux::domain::{
    BrowserInstall, BrowserInstallId, BrowserKind, BrowserProfile, Channel, ExtensionPolicy,
    ProfileId, SupportLevel,
};
use profilemux::error::Error;

fn make_install(kind: BrowserKind, user_data_root: &Path) -> BrowserInstall {
    BrowserInstall {
        id: BrowserInstallId::new(kind, user_data_root),
        kind,
        name: format!("{} Browser", kind.slug()),
        channel: Channel::Stable,
        app_path: PathBuf::from("/Applications/Browser.app"),
        bundle_id: Some("com.example.browser".to_string()),
        version: Some("1.0.0".to_string()),
        user_data_root: user_data_root.to_path_buf(),
        cache_root: None,
        support: SupportLevel::Full,
    }
}

fn make_profile(install: &BrowserInstall, directory: &str, display_name: &str) -> BrowserProfile {
    BrowserProfile {
        id: ProfileId::new(&install.id, directory),
        install_id: install.id.clone(),
        display_name: display_name.to_string(),
        directory: directory.to_string(),
        path: install.user_data_root.join(directory),
        cache_path: None,
        avatar: None,
        last_active: None,
        registered: true,
        directory_exists: true,
        size: None,
    }
}

#[test]
fn test_cli_parse_profile_launch() {
    let cli = Cli::try_parse_from(["pmux", "profile", "launch", "chrome/Default"]).unwrap();
    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Launch(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
        }
        other => panic!("expected Profile Launch, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_profile_create_all_flags() {
    let cli = Cli::try_parse_from([
        "pmux",
        "profile",
        "create",
        "--browser",
        "brave",
        "--name",
        "Work",
        "--directory",
        "Profile 3",
        "--template",
        "Default",
        "--avatar",
        "/tmp/avatar.png",
        "--extensions",
        "copy-settings",
        "--open",
        "--dry-run",
        "--close-browser",
    ])
    .unwrap();

    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Create(args),
        }) => {
            assert_eq!(args.browser, "brave");
            assert_eq!(args.name, "Work");
            assert_eq!(args.directory.as_deref(), Some("Profile 3"));
            assert_eq!(args.template.as_deref(), Some("Default"));
            assert_eq!(args.avatar, Some(PathBuf::from("/tmp/avatar.png")));
            assert_eq!(args.extensions, ExtensionPolicy::CopyExtensionsAndSettings);
            assert!(args.open);
            assert!(args.dry_run);
            assert!(args.close_browser);
        }
        other => panic!("expected Profile Create, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_profile_create_requires_name_and_browser() {
    let err_missing_name =
        Cli::try_parse_from(["pmux", "profile", "create", "--browser", "brave"]).unwrap_err();
    assert!(err_missing_name.to_string().contains("--name"));

    let err_missing_browser =
        Cli::try_parse_from(["pmux", "profile", "create", "--name", "Work"]).unwrap_err();
    assert!(err_missing_browser.to_string().contains("--browser"));
}

#[test]
fn test_cli_parse_profile_clone() {
    let cli = Cli::try_parse_from([
        "pmux",
        "profile",
        "clone",
        "brave/Default",
        "--name",
        "Work Clone",
        "--directory",
        "Profile-Clone",
        "--avatar",
        "/tmp/avatar.png",
        "--extensions",
        "copy",
        "--open",
        "--dry-run",
        "--close-browser",
    ])
    .unwrap();

    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Clone(args),
        }) => {
            assert_eq!(args.selector, "brave/Default");
            assert_eq!(args.name, "Work Clone");
            assert_eq!(args.directory.as_deref(), Some("Profile-Clone"));
            assert_eq!(args.avatar, Some(PathBuf::from("/tmp/avatar.png")));
            assert_eq!(args.extensions, ExtensionPolicy::CopyExtensions);
            assert!(args.open);
            assert!(args.dry_run);
            assert!(args.close_browser);
        }
        other => panic!("expected Profile Clone, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_profile_rename() {
    let cli_name = Cli::try_parse_from([
        "pmux",
        "profile",
        "rename",
        "chrome/Default",
        "--name",
        "New Name",
    ])
    .unwrap();
    match cli_name.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Rename(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
            assert_eq!(args.name.as_deref(), Some("New Name"));
            assert_eq!(args.directory, None);
        }
        other => panic!("expected Profile Rename, got {other:?}"),
    }

    let cli_dir = Cli::try_parse_from([
        "pmux",
        "profile",
        "rename",
        "chrome/Default",
        "--directory",
        "Profile-New",
        "--dry-run",
        "--close-browser",
    ])
    .unwrap();
    match cli_dir.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Rename(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
            assert_eq!(args.name, None);
            assert_eq!(args.directory.as_deref(), Some("Profile-New"));
            assert!(args.dry_run);
            assert!(args.close_browser);
        }
        other => panic!("expected Profile Rename, got {other:?}"),
    }

    let cli_both = Cli::try_parse_from([
        "pmux",
        "profile",
        "rename",
        "chrome/Default",
        "--name",
        "New Name",
        "--directory",
        "Profile-New",
    ])
    .unwrap();
    match cli_both.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Rename(args),
        }) => {
            assert_eq!(args.name.as_deref(), Some("New Name"));
            assert_eq!(args.directory.as_deref(), Some("Profile-New"));
        }
        other => panic!("expected Profile Rename, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_profile_avatar() {
    let cli = Cli::try_parse_from([
        "pmux",
        "profile",
        "avatar",
        "chrome/Default",
        "/tmp/pic.png",
    ])
    .unwrap();
    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Avatar(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
            assert_eq!(args.image, PathBuf::from("/tmp/pic.png"));
        }
        other => panic!("expected Profile Avatar, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_profile_delete() {
    let cli = Cli::try_parse_from([
        "pmux",
        "profile",
        "delete",
        "chrome/Default",
        "--dry-run",
        "--yes",
        "--close-browser",
    ])
    .unwrap();
    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Delete(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
            assert!(args.dry_run);
            assert!(args.yes);
            assert!(args.close_browser);
        }
        other => panic!("expected Profile Delete, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_cache_clean() {
    let cli = Cli::try_parse_from([
        "pmux",
        "cache",
        "clean",
        "chrome/Default",
        "--dry-run",
        "--yes",
        "--close-browser",
    ])
    .unwrap();
    match cli.command {
        Some(Command::Cache {
            command: CacheSubcommand::Clean(args),
        }) => {
            assert_eq!(args.selector, "chrome/Default");
            assert!(args.dry_run);
            assert!(args.yes);
            assert!(args.close_browser);
        }
        other => panic!("expected Cache Clean, got {other:?}"),
    }
}

#[test]
fn test_extension_policy_mapping() {
    assert_eq!(
        parse_extension_policy("none").unwrap(),
        ExtensionPolicy::None
    );
    assert_eq!(
        parse_extension_policy("copy").unwrap(),
        ExtensionPolicy::CopyExtensions
    );
    assert_eq!(
        parse_extension_policy("copy-settings").unwrap(),
        ExtensionPolicy::CopyExtensionsAndSettings
    );
    assert!(parse_extension_policy("invalid").is_err());
    assert!(parse_extension_policy("").is_err());

    let cli_err = Cli::try_parse_from([
        "pmux",
        "profile",
        "create",
        "--browser",
        "chrome",
        "--name",
        "Test",
        "--extensions",
        "invalid",
    ]);
    assert!(cli_err.is_err());
}

#[test]
fn test_create_profile_spec_from_parsed_args() {
    let cli_full = Cli::try_parse_from([
        "pmux",
        "profile",
        "create",
        "--browser",
        "brave",
        "--name",
        "Work Profile",
        "--directory",
        "custom-dir",
        "--template",
        "Template Profile",
        "--avatar",
        "/path/to/avatar.png",
        "--extensions",
        "copy",
        "--open",
    ])
    .unwrap();

    let spec_full = match cli_full.command.unwrap() {
        Command::Profile {
            command: ProfileSubcommand::Create(args),
        } => args.to_spec(),
        _ => panic!("unexpected command"),
    };

    assert_eq!(spec_full.display_name, "Work Profile");
    assert_eq!(spec_full.directory.as_deref(), Some("custom-dir"));
    assert_eq!(
        spec_full.template_directory.as_deref(),
        Some("Template Profile")
    );
    assert_eq!(
        spec_full.avatar_source,
        Some(PathBuf::from("/path/to/avatar.png"))
    );
    assert_eq!(
        spec_full.clone_policy.extensions,
        ExtensionPolicy::CopyExtensions
    );
    assert!(spec_full.clone_policy.copy_preferences);
    assert!(!spec_full.clone_policy.copy_bookmarks);
    assert!(spec_full.open_after_create);

    let cli_minimal = Cli::try_parse_from([
        "pmux",
        "profile",
        "create",
        "--browser",
        "brave",
        "--name",
        "Minimal",
    ])
    .unwrap();

    let spec_minimal = match cli_minimal.command.unwrap() {
        Command::Profile {
            command: ProfileSubcommand::Create(args),
        } => args.to_spec(),
        _ => panic!("unexpected command"),
    };

    assert_eq!(spec_minimal.display_name, "Minimal");
    assert_eq!(spec_minimal.directory, None);
    assert_eq!(spec_minimal.template_directory, None);
    assert_eq!(spec_minimal.avatar_source, None);
    assert_eq!(spec_minimal.clone_policy.extensions, ExtensionPolicy::None);
    assert!(spec_minimal.clone_policy.copy_preferences);
    assert!(!spec_minimal.clone_policy.copy_bookmarks);
    assert!(!spec_minimal.open_after_create);
}

#[test]
fn test_running_browser_preflight_decision() {
    // Refuses when running without --close-browser
    assert_eq!(
        decide_browser_preflight(true, false),
        PreflightDecision::Refuse
    );

    // Proceeds when not running
    assert_eq!(
        decide_browser_preflight(false, false),
        PreflightDecision::Proceed
    );
    assert_eq!(
        decide_browser_preflight(false, true),
        PreflightDecision::Proceed
    );

    // Requests a quit when running with --close-browser
    assert_eq!(
        decide_browser_preflight(true, true),
        PreflightDecision::RequestQuit
    );
}

#[test]
fn test_confirmation_decision() {
    // Refuses without --yes when stdin is not a terminal
    assert_eq!(decide_confirmation(false, false), ConfirmDecision::Refuse);

    // Proceeds with --yes
    assert_eq!(decide_confirmation(true, false), ConfirmDecision::Proceed);
    assert_eq!(decide_confirmation(true, true), ConfirmDecision::Proceed);

    // Prompts without --yes when stdin is a terminal
    assert_eq!(decide_confirmation(false, true), ConfirmDecision::Prompt);
}

#[test]
fn test_resolve_template_in_browser() {
    let root = Path::new("/test/profiles/chrome");
    let install = make_install(BrowserKind::Chrome, root);
    let prof1 = make_profile(&install, "Default", "Personal");
    let prof2 = make_profile(&install, "Profile 1", "Work");
    let profiles = vec![prof1, prof2];

    // Resolve by exact directory
    let matched = resolve_template_in_browser(&install, &profiles, "Default").unwrap();
    assert_eq!(matched.directory, "Default");

    // Resolve by display name (case-insensitive)
    let matched = resolve_template_in_browser(&install, &profiles, "work").unwrap();
    assert_eq!(matched.directory, "Profile 1");

    // Unknown template
    let err = resolve_template_in_browser(&install, &profiles, "nonexistent").unwrap_err();
    assert!(matches!(err, Error::NoSuchProfile(_)));

    // Ambiguous display name within the same browser
    let prof3 = make_profile(&install, "Profile 2", "Work");
    let ambiguous_profiles = vec![profiles[0].clone(), profiles[1].clone(), prof3];
    let err = resolve_template_in_browser(&install, &ambiguous_profiles, "Work").unwrap_err();
    match err {
        Error::AmbiguousSelector {
            selector,
            candidates,
        } => {
            assert_eq!(selector, "Work");
            assert_eq!(candidates, vec!["chrome/Profile 1", "chrome/Profile 2"]);
        }
        other => panic!("expected AmbiguousSelector, got: {other:?}"),
    }
}
