use std::path::Path;

use crate::domain::{
    Appearance, AppearanceCapabilities, AppearanceSpec, BrowserInstall, BrowserKind,
    BrowserProfile, BrowserTheme, OperationKind, OperationPlan, PlanStep, WebDarkMode,
};
use crate::error::{Error, Result};
use crate::fs::Transaction;

fn ensure_stopped(install: &BrowserInstall) -> Result<()> {
    if crate::browsers::chromium::launch::is_running(install) {
        Err(Error::Other(format!(
            "{} is currently running; close the browser before modifying profiles",
            install.name
        )))
    } else {
        Ok(())
    }
}

pub fn capabilities(kind: BrowserKind) -> AppearanceCapabilities {
    let ultra_dark = matches!(
        kind,
        BrowserKind::Brave | BrowserKind::BraveBeta | BrowserKind::BraveNightly
    );
    AppearanceCapabilities {
        browser_theme: true,
        ultra_dark,
        web_dark: true,
    }
}

fn theme_values(theme: BrowserTheme) -> (i64, bool) {
    match theme {
        BrowserTheme::System => (0, false),
        BrowserTheme::Dark => (2, false),
        BrowserTheme::UltraDark => (2, true),
        BrowserTheme::Light | BrowserTheme::Unknown => (0, false),
    }
}

fn extract_theme(doc: &serde_json::Value, is_brave: bool) -> BrowserTheme {
    let Some(obj) = doc.as_object() else {
        return BrowserTheme::Unknown;
    };
    let cs2 = obj
        .get("browser")
        .and_then(|b| b.get("theme"))
        .and_then(|t| t.get("color_scheme2"));

    match cs2 {
        None => BrowserTheme::System,
        Some(v) => match v.as_i64() {
            Some(0) => BrowserTheme::System,
            Some(1) => BrowserTheme::Light,
            Some(2) => {
                let darker = is_brave
                    && obj
                        .get("brave")
                        .and_then(|b| b.get("darker_mode"))
                        .and_then(|d| d.as_bool())
                        .unwrap_or(false);
                if darker {
                    BrowserTheme::UltraDark
                } else {
                    BrowserTheme::Dark
                }
            }
            _ => BrowserTheme::Unknown,
        },
    }
}

pub fn read_appearance(profile_path: &Path, is_brave: bool) -> Result<Appearance> {
    if !profile_path.is_dir() {
        return Err(Error::NotFound(profile_path.to_path_buf()));
    }
    let pref_path = profile_path.join("Preferences");
    let content = match std::fs::read_to_string(&pref_path) {
        Ok(c) => c,
        Err(_) => {
            return Ok(Appearance {
                theme: BrowserTheme::Unknown,
                // Web content dark mode is owned by launch policy, not a profile preference.
                web_dark: WebDarkMode::Normal,
            });
        }
    };
    let doc: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            return Ok(Appearance {
                theme: BrowserTheme::Unknown,
                // Web content dark mode is owned by launch policy, not a profile preference.
                web_dark: WebDarkMode::Normal,
            });
        }
    };

    Ok(Appearance {
        theme: extract_theme(&doc, is_brave),
        // Web content dark mode is owned by launch policy, not a profile preference.
        web_dark: WebDarkMode::Normal,
    })
}

fn validate_spec(spec: &AppearanceSpec, caps: AppearanceCapabilities) -> Result<()> {
    if spec.is_empty() {
        return Err(Error::Other(
            "appearance specification cannot be empty".to_string(),
        ));
    }
    if let Some(theme) = spec.theme {
        match theme {
            BrowserTheme::Light => {
                return Err(Error::Other(
                    "light theme is not a settable option".to_string(),
                ));
            }
            BrowserTheme::Unknown => {
                return Err(Error::Other(
                    "unknown theme is not a settable option".to_string(),
                ));
            }
            BrowserTheme::UltraDark => {
                if !caps.ultra_dark {
                    let reason = caps
                        .reason_unavailable("ultra-dark")
                        .unwrap_or("Brave-only");
                    return Err(Error::Other(format!(
                        "ultra-dark theme is unavailable: {reason}"
                    )));
                }
            }
            BrowserTheme::System | BrowserTheme::Dark => {
                if !caps.browser_theme {
                    let reason = caps.reason_unavailable("theme").unwrap_or("not supported");
                    return Err(Error::Other(format!(
                        "browser theme is unavailable: {reason}"
                    )));
                }
            }
        }
    }
    if spec.web_dark.is_some() && !caps.web_dark {
        let reason = caps
            .reason_unavailable("web-dark")
            .unwrap_or("not supported");
        return Err(Error::Other(format!(
            "web dark mode is unavailable: {reason}"
        )));
    }
    Ok(())
}

fn has_legacy_theme_key(pref_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(pref_path) else {
        return false;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    doc.get("browser")
        .and_then(|b| b.get("theme"))
        .and_then(|t| t.get("color_scheme"))
        .is_some()
}

pub fn plan(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    spec: &AppearanceSpec,
    caps: AppearanceCapabilities,
) -> Result<OperationPlan> {
    validate_spec(spec, caps)?;

    let mut plan = OperationPlan::new(
        OperationKind::SetAppearance,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;

    let pref_path = profile.path.join("Preferences");

    if let Some(theme) = spec.theme {
        plan.paths_affected.push(pref_path.clone());
        let (cs_val, darker_val) = theme_values(theme);

        plan.steps.push(PlanStep::with_detail(
            format!("Set browser.theme.color_scheme2 = {cs_val}"),
            pref_path.display().to_string(),
        ));

        if has_legacy_theme_key(&pref_path) {
            plan.steps.push(PlanStep::with_detail(
                format!("Set browser.theme.color_scheme = {cs_val}"),
                pref_path.display().to_string(),
            ));
        }

        if caps.ultra_dark {
            plan.steps.push(PlanStep::with_detail(
                format!("Set brave.darker_mode = {darker_val}"),
                pref_path.display().to_string(),
            ));
        }
    }

    if spec.web_dark.is_some() {
        // Web-content force dark is owned by launch policy and changes no file.
        plan.steps.push(PlanStep::new(
            "Web dark mode is applied at launch time and changes no file",
        ));
    }

    Ok(plan)
}

fn build_new_browser_obj(
    orig_browser: Option<&serde_json::Map<String, serde_json::Value>>,
    cs_val: i64,
) -> serde_json::Value {
    let mut new_browser = serde_json::Map::new();
    if let Some(bm) = orig_browser {
        for (k, v) in bm {
            if k != "theme" {
                new_browser.insert(k.clone(), v.clone());
            }
        }
    }
    let orig_theme = orig_browser
        .and_then(|b| b.get("theme"))
        .and_then(|t| t.as_object());
    let mut new_theme = serde_json::Map::new();
    let mut had_legacy = false;
    if let Some(tm) = orig_theme {
        for (k, v) in tm {
            if k != "color_scheme2" && k != "color_scheme" && k != "follows_system_colors" {
                new_theme.insert(k.clone(), v.clone());
            } else if k == "color_scheme" {
                had_legacy = true;
            }
        }
    }
    new_theme.insert("color_scheme2".to_string(), serde_json::json!(cs_val));
    if had_legacy {
        new_theme.insert("color_scheme".to_string(), serde_json::json!(cs_val));
    }
    // Verified live: writing only `color_scheme2` is not enough. When the theme
    // still follows the system colours the browser recomputes the scheme on the
    // next launch and resets the value, so an explicit choice has to opt out.
    new_theme.insert(
        "follows_system_colors".to_string(),
        serde_json::json!(cs_val == 0),
    );
    new_browser.insert("theme".to_string(), serde_json::Value::Object(new_theme));
    serde_json::Value::Object(new_browser)
}

fn build_new_brave_obj(
    orig_brave: Option<&serde_json::Map<String, serde_json::Value>>,
    darker_val: bool,
) -> serde_json::Value {
    let mut new_brave = serde_json::Map::new();
    if let Some(brm) = orig_brave {
        for (k, v) in brm {
            if k != "darker_mode" {
                new_brave.insert(k.clone(), v.clone());
            }
        }
    }
    new_brave.insert(
        "darker_mode".to_string(),
        serde_json::Value::Bool(darker_val),
    );
    serde_json::Value::Object(new_brave)
}

fn update_preferences_doc(
    original: &serde_json::Value,
    theme: BrowserTheme,
    is_brave: bool,
) -> serde_json::Value {
    let (cs_val, darker_val) = theme_values(theme);
    let orig_map = original.as_object();
    let mut new_root = serde_json::Map::new();

    if let Some(map) = orig_map {
        for (k, v) in map {
            if k != "browser" && (k != "brave" || !is_brave) {
                new_root.insert(k.clone(), v.clone());
            }
        }
    }

    let orig_browser = orig_map
        .and_then(|m| m.get("browser"))
        .and_then(|b| b.as_object());
    new_root.insert(
        "browser".to_string(),
        build_new_browser_obj(orig_browser, cs_val),
    );

    if is_brave {
        let orig_brave = orig_map
            .and_then(|m| m.get("brave"))
            .and_then(|b| b.as_object());
        new_root.insert(
            "brave".to_string(),
            build_new_brave_obj(orig_brave, darker_val),
        );
    }

    serde_json::Value::Object(new_root)
}

fn check_legacy_theme_pref(doc: &serde_json::Value, expected: i64) -> Result<()> {
    let actual_cs = doc
        .get("browser")
        .and_then(|b| b.get("theme"))
        .and_then(|t| t.get("color_scheme"))
        .and_then(|v| v.as_i64());
    if let Some(cs) = actual_cs {
        if cs != expected {
            return Err(Error::Other(format!(
                "validation failed: expected legacy color_scheme = {expected}, found {cs}"
            )));
        }
    }
    Ok(())
}

fn validate_applied(pref_path: &Path, expected_theme: BrowserTheme, is_brave: bool) -> Result<()> {
    let content = std::fs::read_to_string(pref_path).map_err(|e| Error::io(pref_path, e))?;
    let doc: serde_json::Value = serde_json::from_str(&content).map_err(|e| Error::Malformed {
        path: pref_path.to_path_buf(),
        message: e.to_string(),
    })?;

    let (expected_cs2, expected_darker) = theme_values(expected_theme);
    let actual_cs2 = doc
        .get("browser")
        .and_then(|b| b.get("theme"))
        .and_then(|t| t.get("color_scheme2"))
        .and_then(|v| v.as_i64());

    if actual_cs2 != Some(expected_cs2) {
        return Err(Error::Other(format!(
            "validation failed: expected color_scheme2 = {expected_cs2}, found {actual_cs2:?}"
        )));
    }

    check_legacy_theme_pref(&doc, expected_cs2)?;

    if is_brave {
        let actual_darker = doc
            .get("brave")
            .and_then(|b| b.get("darker_mode"))
            .and_then(|v| v.as_bool());
        if actual_darker != Some(expected_darker) {
            return Err(Error::Other(format!(
                "validation failed: expected brave.darker_mode = {expected_darker}, found {actual_darker:?}"
            )));
        }
    }

    Ok(())
}

pub fn apply(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    spec: &AppearanceSpec,
    caps: AppearanceCapabilities,
) -> Result<()> {
    validate_spec(spec, caps)?;
    ensure_stopped(install)?;

    if !profile.path.is_dir() {
        return Err(Error::NotFound(profile.path.clone()));
    }

    let Some(theme) = spec.theme else {
        // Web-content force dark is owned by launch policy and changes no file.
        return Ok(());
    };

    let pref_path = profile.path.join("Preferences");
    let original_doc = match std::fs::read_to_string(&pref_path) {
        Ok(c) => serde_json::from_str::<serde_json::Value>(&c).map_err(|e| Error::Malformed {
            path: pref_path.clone(),
            message: e.to_string(),
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            serde_json::Value::Object(serde_json::Map::new())
        }
        Err(e) => return Err(Error::io(&pref_path, e)),
    };

    let new_doc = update_preferences_doc(&original_doc, theme, caps.ultra_dark);
    let bytes = serde_json::to_vec_pretty(&new_doc).map_err(|e| Error::Malformed {
        path: pref_path.clone(),
        message: e.to_string(),
    })?;

    let mut tx = Transaction::new("set_appearance")?;
    if pref_path.exists() {
        tx.backup_file(&pref_path)?;
    }
    if let Err(err) = tx.write_file(&pref_path, &bytes) {
        let _ = tx.rollback();
        return Err(err);
    }
    if let Err(err) = validate_applied(&pref_path, theme, caps.ultra_dark) {
        let _ = tx.rollback();
        return Err(err);
    }
    tx.commit()
}
