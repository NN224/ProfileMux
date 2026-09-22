use clap::Parser;
use profilemux::cli::appearance_cmd::{
    build_spec, check_supported, render_inspect, render_web_dark_dry_run,
};
use profilemux::cli::commands::{Cli, Command, ProfileSubcommand};
use profilemux::domain::{
    Appearance, AppearanceCapabilities, AppearanceSpec, BrowserTheme, WebDarkMode,
};

#[test]
fn test_cli_parse_appearance_inspect() {
    let cli = Cli::try_parse_from(["pmux", "profile", "appearance", "my-profile"]).unwrap();
    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Appearance(args),
        }) => {
            assert_eq!(args.selector, "my-profile");
            assert!(args.theme.is_none());
            assert!(args.web_dark.is_none());
            assert!(!args.dry_run);
            assert!(!args.close_browser);
            assert!(!args.yes);
        }
        other => panic!("expected Profile Appearance, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_appearance_theme_flags() {
    for theme in ["system", "dark", "ultra-dark"] {
        let cli =
            Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--theme", theme]).unwrap();
        match cli.command {
            Some(Command::Profile {
                command: ProfileSubcommand::Appearance(args),
            }) => {
                assert_eq!(args.selector, "p1");
                assert_eq!(args.theme.as_deref(), Some(theme));
                assert!(args.web_dark.is_none());
            }
            other => panic!("expected Profile Appearance, got {other:?}"),
        }
    }
}

#[test]
fn test_cli_parse_appearance_web_dark_flags() {
    for web_dark in ["off", "force"] {
        let cli = Cli::try_parse_from([
            "pmux",
            "profile",
            "appearance",
            "p1",
            "--web-dark",
            web_dark,
        ])
        .unwrap();
        match cli.command {
            Some(Command::Profile {
                command: ProfileSubcommand::Appearance(args),
            }) => {
                assert_eq!(args.selector, "p1");
                assert!(args.theme.is_none());
                assert_eq!(args.web_dark.as_deref(), Some(web_dark));
            }
            other => panic!("expected Profile Appearance, got {other:?}"),
        }
    }
}

#[test]
fn test_cli_parse_appearance_operational_flags() {
    let dry_run =
        Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--dry-run"]).unwrap();
    let close =
        Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--close-browser"]).unwrap();
    let yes = Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--yes"]).unwrap();

    let extract = |cli: Cli| match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Appearance(args),
        }) => (args.dry_run, args.close_browser, args.yes),
        other => panic!("expected Profile Appearance, got {other:?}"),
    };

    assert_eq!(extract(dry_run), (true, false, false));
    assert_eq!(extract(close), (false, true, false));
    assert_eq!(extract(yes), (false, false, true));
}

#[test]
fn test_cli_parse_appearance_all_flags_together() {
    let cli = Cli::try_parse_from([
        "pmux",
        "profile",
        "appearance",
        "p1",
        "--theme",
        "ultra-dark",
        "--web-dark",
        "force",
        "--dry-run",
        "--close-browser",
        "--yes",
    ])
    .unwrap();

    match cli.command {
        Some(Command::Profile {
            command: ProfileSubcommand::Appearance(args),
        }) => {
            assert_eq!(args.selector, "p1");
            assert_eq!(args.theme.as_deref(), Some("ultra-dark"));
            assert_eq!(args.web_dark.as_deref(), Some("force"));
            assert!(args.dry_run);
            assert!(args.close_browser);
            assert!(args.yes);
        }
        other => panic!("expected Profile Appearance, got {other:?}"),
    }
}

#[test]
fn test_cli_parse_appearance_rejected_values() {
    assert!(
        Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--theme", "light"]).is_err()
    );
    assert!(
        Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--theme", "bogus"]).is_err()
    );
    assert!(
        Cli::try_parse_from(["pmux", "profile", "appearance", "p1", "--web-dark", "bogus"])
            .is_err()
    );
}

#[test]
fn test_build_spec_valid_mappings() {
    let spec = build_spec(Some("system"), None).unwrap();
    assert_eq!(spec.theme, Some(BrowserTheme::System));
    assert_eq!(spec.web_dark, None);

    let spec = build_spec(Some("dark"), None).unwrap();
    assert_eq!(spec.theme, Some(BrowserTheme::Dark));
    assert_eq!(spec.web_dark, None);

    let spec = build_spec(Some("ultra-dark"), None).unwrap();
    assert_eq!(spec.theme, Some(BrowserTheme::UltraDark));
    assert_eq!(spec.web_dark, None);

    let spec = build_spec(None, Some("off")).unwrap();
    assert_eq!(spec.theme, None);
    assert_eq!(spec.web_dark, Some(WebDarkMode::Normal));

    let spec = build_spec(None, Some("force")).unwrap();
    assert_eq!(spec.theme, None);
    assert_eq!(spec.web_dark, Some(WebDarkMode::ForceDark));

    let spec = build_spec(Some("ultra-dark"), Some("force")).unwrap();
    assert_eq!(spec.theme, Some(BrowserTheme::UltraDark));
    assert_eq!(spec.web_dark, Some(WebDarkMode::ForceDark));

    let spec = build_spec(None, None).unwrap();
    assert!(spec.is_empty());
}

#[test]
fn test_build_spec_invalid_mappings() {
    assert!(build_spec(Some("light"), None).is_err());
    assert!(build_spec(Some("bogus"), None).is_err());
    assert!(build_spec(None, Some("bogus")).is_err());
    assert!(build_spec(Some("bogus"), Some("force")).is_err());
    assert!(build_spec(Some("dark"), Some("bogus")).is_err());
}

#[test]
fn test_render_inspect_shows_labels_and_values() {
    let app1 = Appearance {
        theme: BrowserTheme::UltraDark,
        web_dark: WebDarkMode::Normal,
    };
    let output1 = render_inspect("ONE CLUB", &app1);
    assert!(output1.contains("Profile:           ONE CLUB"));
    assert!(output1.contains("Browser Theme:     Ultra Dark"));
    assert!(output1.contains("Web Dark Mode:     Off"));

    let app2 = Appearance {
        theme: BrowserTheme::Unknown,
        web_dark: WebDarkMode::ForceDark,
    };
    let output2 = render_inspect("Unknown Theme Profile", &app2);
    assert!(output2.contains("Browser Theme:     Unknown"));
    assert!(output2.contains("Web Dark Mode:     Force Dark (Experimental)"));
}

#[test]
fn test_render_inspect_value_column_alignment() {
    let appearance = Appearance {
        theme: BrowserTheme::UltraDark,
        web_dark: WebDarkMode::Normal,
    };
    let output = render_inspect("TEST PROFILE", &appearance);
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 3);
    let value_columns: Vec<usize> = lines
        .iter()
        .map(|line| {
            let colon_idx = line.find(':').expect("label line must contain colon");
            let after_colon = &line[colon_idx + 1..];
            let space_count = after_colon.len() - after_colon.trim_start().len();
            colon_idx + 1 + space_count
        })
        .collect();

    let expected_col = value_columns[0];
    assert_eq!(expected_col, 19, "value column should start at index 19");
    for col in value_columns {
        assert_eq!(col, expected_col, "column alignment regressed");
    }
}

#[test]
fn test_check_supported_ultra_dark() {
    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::UltraDark),
        web_dark: None,
    };

    let caps_without_ultra = AppearanceCapabilities {
        browser_theme: true,
        ultra_dark: false,
        web_dark: true,
    };
    let err = check_supported(&spec, caps_without_ultra).unwrap_err();
    assert!(
        err.to_string().contains("Brave-only"),
        "expected Brave-only in error message, got: {err}"
    );

    let caps_with_ultra = AppearanceCapabilities {
        browser_theme: true,
        ultra_dark: true,
        web_dark: true,
    };
    assert!(check_supported(&spec, caps_with_ultra).is_ok());
}

#[test]
fn test_check_supported_web_dark() {
    let spec = AppearanceSpec {
        theme: None,
        web_dark: Some(WebDarkMode::ForceDark),
    };
    let caps_without_web_dark = AppearanceCapabilities {
        browser_theme: true,
        ultra_dark: true,
        web_dark: false,
    };
    let err = check_supported(&spec, caps_without_web_dark).unwrap_err();
    assert!(err.to_string().contains("not supported by this browser"));
}

#[test]
fn test_check_supported_empty_spec() {
    let spec = AppearanceSpec::default();
    let caps = AppearanceCapabilities {
        browser_theme: true,
        ultra_dark: true,
        web_dark: true,
    };
    assert!(check_supported(&spec, caps).is_err());
}

#[test]
fn test_render_web_dark_dry_run() {
    let temp = tempfile::tempdir().unwrap();
    let policy_path = temp.path().join("policy.json");
    let output = render_web_dark_dry_run(&policy_path, WebDarkMode::Normal, WebDarkMode::ForceDark);

    assert!(output.contains(&policy_path.display().to_string()));
    assert!(output.contains("Off"));
    assert!(output.contains("Force Dark (Experimental)"));

    let lower = output.to_lowercase();
    assert!(!lower.contains("transaction"));
    assert!(!lower.contains("commit"));
    assert!(!lower.contains("rollback"));
}
