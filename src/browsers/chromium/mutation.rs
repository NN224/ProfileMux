use std::path::{Path, PathBuf};

use crate::domain::{
    AvatarInfo, BrowserInstall, BrowserProfile, ClonePolicy, CloneProfileSpec, CreateProfileSpec,
    DeleteMode, OperationKind, OperationPlan, PlanStep, ProfileId,
};
use crate::error::{Error, Result};
use crate::fs::transaction::Transaction;
use crate::fs::trash::TrashBin;

pub const CACHE_SUBDIRS: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "ShaderCache",
    "GrShaderCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "component_crx_cache",
];

pub fn ensure_stopped(install: &BrowserInstall) -> Result<()> {
    if crate::browsers::chromium::launch::is_running(install) {
        Err(Error::Other(format!(
            "{} is currently running; close the browser before modifying profiles",
            install.name
        )))
    } else {
        Ok(())
    }
}

pub fn read_local_state(install: &BrowserInstall) -> Result<serde_json::Value> {
    let path = install.user_data_root.join("Local State");
    match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).map_err(|e| Error::Malformed {
            path,
            message: e.to_string(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::Value::Object(serde_json::Map::new()))
        }
        Err(e) => Err(Error::io(path, e)),
    }
}

fn write_local_state(
    tx: &mut Transaction,
    install: &BrowserInstall,
    doc: &serde_json::Value,
) -> Result<()> {
    let path = install.user_data_root.join("Local State");
    if path.exists() {
        tx.backup_file(&path)?;
    }
    let bytes = serde_json::to_vec_pretty(doc).map_err(|e| Error::Malformed {
        path: path.clone(),
        message: e.to_string(),
    })?;
    tx.write_file(&path, &bytes)
}

pub fn next_free_directory(root: &Path, existing: &[String]) -> String {
    let mut n = 1;
    loop {
        let candidate = format!("Profile {n}");
        if !root.join(&candidate).exists() && !existing.iter().any(|e| e == &candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn resolve_directory(
    spec_dir: Option<&str>,
    display_name: &str,
    existing: &[String],
    root: &Path,
) -> Result<String> {
    let candidate = match spec_dir {
        Some(dir) => {
            if !crate::domain::sanitize::is_safe_component(dir) {
                return Err(Error::Other(format!(
                    "directory name `{dir}` is unsafe: contains forbidden characters or is reserved"
                )));
            }
            dir.to_string()
        }
        None => match crate::domain::sanitize::sanitize_directory_name(display_name) {
            Some(sanitized) => crate::domain::sanitize::deduplicate(&sanitized, existing),
            None => next_free_directory(root, existing),
        },
    };
    let joined = root.join(&candidate);
    if joined.parent() != Some(root) {
        return Err(Error::Other(format!(
            "directory `{candidate}` escapes user data root"
        )));
    }
    Ok(candidate)
}

fn extract_existing_directories(local_state: &serde_json::Value, root: &Path) -> Vec<String> {
    let mut dirs = Vec::new();
    let ic_opt = local_state
        .get("profile")
        .and_then(|p| p.get("info_cache"))
        .and_then(|ic| ic.as_object());
    if let Some(ic) = ic_opt {
        for k in ic.keys() {
            dirs.push(k.clone());
        }
    }
    if let Ok(rd) = std::fs::read_dir(root) {
        for entry in rd.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if !dirs.iter().any(|d| d == name) {
                        dirs.push(name.to_string());
                    }
                }
            }
        }
    }
    dirs
}

fn standard_private_areas() -> Vec<String> {
    ["Cookies", "Login Data", "History", "Sessions", "Extensions"]
        .into_iter()
        .map(String::from)
        .collect()
}

pub fn plan_create(install: &BrowserInstall, spec: &CreateProfileSpec) -> Result<OperationPlan> {
    plan_create_internal(install, spec, OperationKind::CreateProfile)
}

fn plan_create_internal(
    install: &BrowserInstall,
    spec: &CreateProfileSpec,
    kind: OperationKind,
) -> Result<OperationPlan> {
    let ls = read_local_state(install)?;
    let existing = extract_existing_directories(&ls, &install.user_data_root);
    let dir = resolve_directory(
        spec.directory.as_deref(),
        &spec.display_name,
        &existing,
        &install.user_data_root,
    )?;
    let profile_path = install.user_data_root.join(&dir);
    let local_state_path = install.user_data_root.join("Local State");

    let mut plan = OperationPlan::new(kind, install.label(), &spec.display_name);
    plan.requires_browser_closed = true;
    plan.steps.push(PlanStep::with_detail(
        "Create profile directory",
        profile_path.display().to_string(),
    ));
    if let Some(tpl) = &spec.template_directory {
        plan.steps.push(PlanStep::with_detail(
            "Copy settings from template",
            tpl.clone(),
        ));
        plan.copies = crate::browsers::chromium::clone::copied_labels(&spec.clone_policy);
        plan.excludes = crate::browsers::chromium::clone::excluded_labels(&spec.clone_policy);
    } else {
        plan.excludes = standard_private_areas();
    }
    if let Some(av) = &spec.avatar_source {
        plan.steps.push(PlanStep::with_detail(
            "Install custom avatar",
            av.display().to_string(),
        ));
    }
    plan.steps.push(PlanStep::with_detail(
        "Register profile in Local State",
        format!("info_cache.{dir}"),
    ));
    plan.paths_affected = vec![profile_path, local_state_path];
    Ok(plan)
}

fn copy_template_items(
    tx: &mut Transaction,
    tpl_path: &Path,
    profile_path: &Path,
    policy: &ClonePolicy,
) -> Result<()> {
    let items = crate::browsers::chromium::clone::plan_copy_items(tpl_path, policy);
    let mut has_preferences = false;
    for item in items {
        let src = tpl_path.join(&item.relative);
        let dst = profile_path.join(&item.relative);
        if item.is_dir {
            tx.copy_dir(&src, &dst)?;
        } else if item.relative == Path::new("Preferences") {
            has_preferences = true;
            let sanitized = crate::browsers::chromium::clone::load_and_sanitize(&src)?;
            tx.write_file(&dst, &sanitized)?;
        } else {
            tx.copy_file(&src, &dst)?;
        }
    }
    if !has_preferences {
        tx.write_file(&profile_path.join("Preferences"), b"{}")?;
    }
    Ok(())
}

fn install_avatar(tx: &mut Transaction, avatar_source: &Path, profile_path: &Path) -> Result<()> {
    let bytes = crate::browsers::chromium::avatar::normalize_to_png(
        avatar_source,
        crate::browsers::chromium::avatar::MAX_DIMENSION,
    )?;
    tx.write_file(
        &profile_path.join(crate::browsers::chromium::avatar::AVATAR_FILE_NAME),
        &bytes,
    )
}

fn update_local_state_for_create(
    local_state: &mut serde_json::Value,
    dir: &str,
    display_name: &str,
    template_dir: Option<&str>,
    has_avatar: bool,
) -> Result<serde_json::Value> {
    if local_state.get("profile").is_none() {
        local_state["profile"] = serde_json::json!({});
    }
    let p_obj = local_state
        .get_mut("profile")
        .and_then(|p| p.as_object_mut())
        .ok_or_else(|| Error::Other("profile is not a JSON object".to_string()))?;

    let mut entry = serde_json::Map::new();
    entry.insert(
        "name".into(),
        serde_json::Value::String(display_name.to_string()),
    );
    entry.insert(
        "is_using_default_name".into(),
        serde_json::Value::Bool(false),
    );

    if let Some(tpl) = template_dir {
        if let Some(icon) = p_obj
            .get("info_cache")
            .and_then(|ic| ic.get(tpl))
            .and_then(|e| e.get("avatar_icon"))
        {
            entry.insert("avatar_icon".into(), icon.clone());
        }
    }
    if has_avatar {
        for (k, v) in crate::browsers::chromium::avatar::local_state_avatar_fields() {
            entry.insert(k.to_string(), v);
        }
    }

    if p_obj.get("info_cache").is_none() {
        p_obj.insert("info_cache".into(), serde_json::json!({}));
    }
    let ic = p_obj
        .get_mut("info_cache")
        .and_then(|ic| ic.as_object_mut())
        .ok_or_else(|| Error::Other("info_cache is not a JSON object".to_string()))?;

    let entry_val = serde_json::Value::Object(entry);
    ic.insert(dir.to_string(), entry_val.clone());
    if let Some(po) = p_obj
        .get_mut("profiles_order")
        .and_then(|po| po.as_array_mut())
    {
        po.push(serde_json::Value::String(dir.to_string()));
    }
    Ok(entry_val)
}

pub fn create_profile(
    install: &BrowserInstall,
    spec: &CreateProfileSpec,
) -> Result<BrowserProfile> {
    ensure_stopped(install)?;
    let mut local_state = read_local_state(install)?;
    let existing = extract_existing_directories(&local_state, &install.user_data_root);
    let dir = resolve_directory(
        spec.directory.as_deref(),
        &spec.display_name,
        &existing,
        &install.user_data_root,
    )?;
    let profile_path = install.user_data_root.join(&dir);
    if profile_path.exists() {
        return Err(Error::Other(format!(
            "directory already exists: {}",
            profile_path.display()
        )));
    }

    let mut tx = Transaction::new("create_profile")?;
    tx.create_dir(&profile_path)?;

    if let Some(tpl_name) = &spec.template_directory {
        let tpl_path = install.user_data_root.join(tpl_name);
        if !tpl_path.is_dir() {
            return Err(Error::NotFound(tpl_path));
        }
        copy_template_items(&mut tx, &tpl_path, &profile_path, &spec.clone_policy)?;
    } else {
        tx.write_file(&profile_path.join("Preferences"), b"{}")?;
    }

    if let Some(avatar_src) = &spec.avatar_source {
        install_avatar(&mut tx, avatar_src, &profile_path)?;
    }

    let entry = update_local_state_for_create(
        &mut local_state,
        &dir,
        &spec.display_name,
        spec.template_directory.as_deref(),
        spec.avatar_source.is_some(),
    )?;

    write_local_state(&mut tx, install, &local_state)?;
    tx.commit()?;
    Ok(build_profile_snapshot(install, &dir, &entry))
}

pub fn plan_clone(
    install: &BrowserInstall,
    source: &BrowserProfile,
    spec: &CloneProfileSpec,
) -> Result<OperationPlan> {
    let create_spec = CreateProfileSpec {
        display_name: spec.display_name.clone(),
        directory: spec.directory.clone(),
        template_directory: Some(source.directory.clone()),
        avatar_source: spec.avatar_source.clone(),
        clone_policy: spec.clone_policy,
        open_after_create: spec.open_after_create,
    };
    plan_create_internal(install, &create_spec, OperationKind::CloneProfile)
}

pub fn clone_profile(
    install: &BrowserInstall,
    source: &BrowserProfile,
    spec: &CloneProfileSpec,
) -> Result<BrowserProfile> {
    let create_spec = CreateProfileSpec {
        display_name: spec.display_name.clone(),
        directory: spec.directory.clone(),
        template_directory: Some(source.directory.clone()),
        avatar_source: spec.avatar_source.clone(),
        clone_policy: spec.clone_policy,
        open_after_create: spec.open_after_create,
    };
    create_profile(install, &create_spec)
}

pub fn plan_rename_display_name(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    new_name: &str,
) -> Result<OperationPlan> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(Error::Other(
            "new display name cannot be empty or whitespace-only".to_string(),
        ));
    }
    let mut plan = OperationPlan::new(
        OperationKind::RenameDisplayName,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;
    plan.steps.push(PlanStep::with_detail(
        "Rename display name",
        format!("\"{}\" -> \"{}\"", profile.display_name, trimmed),
    ));
    plan.paths_affected = vec![install.user_data_root.join("Local State")];
    Ok(plan)
}

pub fn rename_display_name(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    new_name: &str,
) -> Result<()> {
    ensure_stopped(install)?;
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(Error::Other(
            "new display name cannot be empty or whitespace-only".to_string(),
        ));
    }
    let mut local_state = read_local_state(install)?;
    let entry = local_state
        .get_mut("profile")
        .and_then(|p| p.get_mut("info_cache"))
        .and_then(|ic| ic.get_mut(&profile.directory))
        .and_then(|e| e.as_object_mut())
        .ok_or_else(|| Error::NoSuchProfile(profile.directory.clone()))?;

    entry.insert(
        "name".to_string(),
        serde_json::Value::String(trimmed.to_string()),
    );
    entry.insert(
        "is_using_default_name".to_string(),
        serde_json::Value::Bool(false),
    );

    let mut tx = Transaction::new("rename_display_name")?;
    write_local_state(&mut tx, install, &local_state)?;
    tx.commit()
}

pub fn plan_set_avatar(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    image: &Path,
) -> Result<OperationPlan> {
    let mut plan = OperationPlan::new(
        OperationKind::SetAvatar,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;
    plan.steps.push(PlanStep::with_detail(
        "Install avatar image",
        format!(
            "{} from {}",
            crate::browsers::chromium::avatar::AVATAR_FILE_NAME,
            image.display()
        ),
    ));
    plan.steps.push(PlanStep::with_detail(
        "Update profile avatar metadata",
        format!("info_cache.{}", profile.directory),
    ));
    plan.paths_affected = vec![
        profile
            .path
            .join(crate::browsers::chromium::avatar::AVATAR_FILE_NAME),
        install.user_data_root.join("Local State"),
    ];
    Ok(plan)
}

pub fn set_avatar(install: &BrowserInstall, profile: &BrowserProfile, image: &Path) -> Result<()> {
    ensure_stopped(install)?;
    if !image.is_file() {
        return Err(Error::NotFound(image.to_path_buf()));
    }
    let png_bytes = crate::browsers::chromium::avatar::normalize_to_png(
        image,
        crate::browsers::chromium::avatar::MAX_DIMENSION,
    )?;
    let mut local_state = read_local_state(install)?;
    let entry = local_state
        .get_mut("profile")
        .and_then(|p| p.get_mut("info_cache"))
        .and_then(|ic| ic.get_mut(&profile.directory))
        .and_then(|e| e.as_object_mut())
        .ok_or_else(|| Error::NoSuchProfile(profile.directory.clone()))?;

    for (k, v) in crate::browsers::chromium::avatar::local_state_avatar_fields() {
        entry.insert(k.to_string(), v);
    }

    let mut tx = Transaction::new("set_avatar")?;
    let target = profile
        .path
        .join(crate::browsers::chromium::avatar::AVATAR_FILE_NAME);
    if target.exists() {
        tx.backup_file(&target)?;
    }
    tx.write_file(&target, &png_bytes)?;
    write_local_state(&mut tx, install, &local_state)?;
    tx.commit()
}

pub fn plan_delete(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    _mode: DeleteMode,
) -> Result<OperationPlan> {
    let mut plan = OperationPlan::new(
        OperationKind::DeleteProfile,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;
    let breakdown = crate::fs::size::measure_profile(&profile.path, profile.cache_path.as_deref());
    plan.reclaimed_bytes = Some(breakdown.total);

    plan.steps.push(PlanStep::new(format!(
        "Send profile directory to Trash: {}",
        profile.path.display()
    )));
    let mut paths = vec![profile.path.clone()];
    if let Some(cp) = &profile.cache_path {
        if cp.exists() {
            plan.steps.push(PlanStep::new(format!(
                "Send cache directory to Trash: {}",
                cp.display()
            )));
            paths.push(cp.clone());
        }
    }
    plan.steps.push(PlanStep::new(format!(
        "Remove profile `{}` from Local State info_cache and profiles_order",
        profile.directory
    )));
    plan.paths_affected = paths;
    Ok(plan)
}

pub fn delete_profile(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    _mode: DeleteMode,
) -> Result<()> {
    delete_profile_with(install, profile, &crate::fs::trash::SystemTrash)
}

pub fn delete_profile_with(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    trash: &dyn TrashBin,
) -> Result<()> {
    ensure_stopped(install)?;
    let mut local_state = read_local_state(install)?;
    if let Some(p_obj) = local_state
        .get_mut("profile")
        .and_then(|p| p.as_object_mut())
    {
        if let Some(ic) = p_obj
            .get_mut("info_cache")
            .and_then(|ic| ic.as_object_mut())
        {
            ic.remove(&profile.directory);
        }
        if let Some(po) = p_obj
            .get_mut("profiles_order")
            .and_then(|po| po.as_array_mut())
        {
            po.retain(|item| item.as_str() != Some(&profile.directory));
        }
    }
    let mut tx = Transaction::new("delete_profile")?;
    write_local_state(&mut tx, install, &local_state)?;
    tx.commit()?;

    if profile.path.exists() {
        trash.send(&profile.path)?;
    }
    if let Some(cp) = &profile.cache_path {
        if cp.exists() {
            trash.send(cp)?;
        }
    } else if let Some(cr) = &install.cache_root {
        let cp = cr.join(&profile.directory);
        if cp.exists() {
            trash.send(&cp)?;
        }
    }
    Ok(())
}

pub fn cleanable_paths(profile_path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for sub in CACHE_SUBDIRS {
        paths.push(profile_path.join(sub));
    }
    paths.push(profile_path.join("Service Worker").join("CacheStorage"));
    paths
}

pub fn measure_cleanable_cache(
    profile_path: &Path,
    cache_root: Option<&Path>,
    directory: &str,
) -> u64 {
    let mut total: u64 = 0;
    if let Some(cr) = cache_root {
        let ext = cr.join(directory);
        if ext.exists() {
            total = total.saturating_add(crate::fs::size::dir_size(&ext));
        }
    }
    for p in cleanable_paths(profile_path) {
        if p.exists() {
            total = total.saturating_add(crate::fs::size::dir_size(&p));
        }
    }
    total
}

pub fn plan_clean_cache(
    install: &BrowserInstall,
    profile: &BrowserProfile,
) -> Result<OperationPlan> {
    let mut plan = OperationPlan::new(
        OperationKind::CleanCache,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;
    let bytes = measure_cleanable_cache(
        &profile.path,
        install.cache_root.as_deref(),
        &profile.directory,
    );
    plan.reclaimed_bytes = Some(bytes);
    plan.steps
        .push(PlanStep::new("Clean verified cache directories"));
    let mut paths = Vec::new();
    if let Some(cr) = &install.cache_root {
        let ext = cr.join(&profile.directory);
        if ext.exists() {
            paths.push(ext);
        }
    }
    for p in cleanable_paths(&profile.path) {
        if p.exists() {
            paths.push(p);
        }
    }
    plan.paths_affected = paths;
    Ok(plan)
}

pub fn clean_cache(install: &BrowserInstall, profile: &BrowserProfile) -> Result<u64> {
    ensure_stopped(install)?;
    let mut reclaimed: u64 = 0;

    if let Some(cr) = &install.cache_root {
        let ext = cr.join(&profile.directory);
        if ext.exists() {
            reclaimed = reclaimed.saturating_add(crate::fs::size::dir_size(&ext));
            match std::fs::remove_dir_all(&ext) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::io(ext, e)),
            }
        }
    }

    for p in cleanable_paths(&profile.path) {
        if p.exists() {
            reclaimed = reclaimed.saturating_add(crate::fs::size::dir_size(&p));
            let res = if p.is_dir() {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
            match res {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::io(p, e)),
            }
        }
    }
    Ok(reclaimed)
}

pub fn plan_rename_directory(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    new_directory: &str,
) -> Result<OperationPlan> {
    if !crate::domain::sanitize::is_safe_component(new_directory) {
        return Err(Error::Other(format!(
            "new directory name `{new_directory}` is unsafe: contains forbidden characters, starts with dot, or is reserved"
        )));
    }
    let new_path = install.user_data_root.join(new_directory);
    let mut plan = OperationPlan::new(
        OperationKind::RenameDirectory,
        install.label(),
        &profile.display_name,
    );
    plan.requires_browser_closed = true;
    plan.steps.push(PlanStep::with_detail(
        "Rename profile directory",
        format!("{} -> {}", profile.directory, new_directory),
    ));
    let mut paths = vec![profile.path.clone(), new_path];
    if let Some(cr) = &install.cache_root {
        let old_cache = cr.join(&profile.directory);
        let new_cache = cr.join(new_directory);
        if old_cache.exists() {
            plan.steps.push(PlanStep::with_detail(
                "Rename cache directory",
                format!("{} -> {}", profile.directory, new_directory),
            ));
            paths.push(old_cache);
            paths.push(new_cache);
        }
    }
    plan.steps.push(PlanStep::with_detail(
        "Move Local State info_cache key",
        format!("{} -> {}", profile.directory, new_directory),
    ));
    plan.steps.push(PlanStep::with_detail(
        "Rewrite profiles_order entry",
        format!("{} -> {}", profile.directory, new_directory),
    ));
    paths.push(install.user_data_root.join("Local State"));
    plan.paths_affected = paths;
    Ok(plan)
}

fn update_local_state_for_rename(
    local_state: &mut serde_json::Value,
    old_dir: &str,
    new_dir: &str,
    install_root: &Path,
) -> Result<()> {
    let p_obj = local_state
        .get_mut("profile")
        .and_then(|p| p.get_mut("info_cache"))
        .and_then(|ic| ic.as_object_mut())
        .ok_or_else(|| Error::Malformed {
            path: install_root.join("Local State"),
            message: "missing profile.info_cache in Local State".to_string(),
        })?;
    let entry = match p_obj.remove(old_dir) {
        Some(e) => e,
        None => return Err(Error::NoSuchProfile(old_dir.to_string())),
    };
    p_obj.insert(new_dir.to_string(), entry);

    if let Some(po) = local_state
        .get_mut("profile")
        .and_then(|p| p.get_mut("profiles_order"))
        .and_then(|po| po.as_array_mut())
    {
        for item in po.iter_mut() {
            if item.as_str() == Some(old_dir) {
                *item = serde_json::Value::String(new_dir.to_string());
            }
        }
    }
    Ok(())
}

pub fn rename_profile_directory(
    install: &BrowserInstall,
    profile: &BrowserProfile,
    new_directory: &str,
) -> Result<()> {
    ensure_stopped(install)?;
    if !crate::domain::sanitize::is_safe_component(new_directory) {
        return Err(Error::Other(format!(
            "new directory name `{new_directory}` is unsafe: contains forbidden characters, starts with dot, or is reserved"
        )));
    }
    if new_directory == profile.directory {
        return Ok(());
    }
    let new_profile_path = install.user_data_root.join(new_directory);
    if new_profile_path.exists() {
        return Err(Error::Other(format!(
            "directory `{}` already exists",
            new_profile_path.display()
        )));
    }
    if new_profile_path.parent() != Some(&install.user_data_root) {
        return Err(Error::Other(format!(
            "directory `{new_directory}` escapes user data root"
        )));
    }

    let mut local_state = read_local_state(install)?;
    let mut tx = Transaction::new("rename_directory")?;
    tx.rename(&profile.path, &new_profile_path)?;

    if let Some(cr) = &install.cache_root {
        let old_cache = cr.join(&profile.directory);
        let new_cache = cr.join(new_directory);
        if old_cache.exists() {
            tx.rename(&old_cache, &new_cache)?;
        }
    }

    update_local_state_for_rename(
        &mut local_state,
        &profile.directory,
        new_directory,
        &install.user_data_root,
    )?;
    write_local_state(&mut tx, install, &local_state)?;

    if let Err(e) = validate_rename(&install.user_data_root, new_directory, &profile.directory) {
        tx.rollback()?;
        return Err(e);
    }
    tx.commit()
}

fn validate_rename(user_data_root: &Path, new_directory: &str, old_directory: &str) -> Result<()> {
    let new_profile_path = user_data_root.join(new_directory);
    if !new_profile_path.is_dir() {
        return Err(Error::Other(format!(
            "validation failed: new directory `{}` does not exist on disk",
            new_profile_path.display()
        )));
    }
    let local_state_path = user_data_root.join("Local State");
    let content = match std::fs::read_to_string(&local_state_path) {
        Ok(c) => c,
        Err(e) => return Err(Error::io(&local_state_path, e)),
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::Malformed {
                path: local_state_path,
                message: e.to_string(),
            });
        }
    };
    let ic = val
        .get("profile")
        .and_then(|p| p.get("info_cache"))
        .and_then(|ic| ic.as_object());
    match ic {
        Some(map) => {
            if !map.contains_key(new_directory) || map.contains_key(old_directory) {
                return Err(Error::Other(
                    "validation failed: Local State info_cache keys incorrect after rename"
                        .to_string(),
                ));
            }
        }
        None => {
            return Err(Error::Other(
                "validation failed: missing info_cache in Local State".to_string(),
            ));
        }
    }
    Ok(())
}

fn build_avatar_from_entry(entry: &serde_json::Value) -> Option<AvatarInfo> {
    if entry.get("avatar_icon").is_none()
        && entry.get("gaia_picture_file_name").is_none()
        && entry.get("use_gaia_picture").is_none()
        && entry.get("is_using_default_avatar").is_none()
    {
        return None;
    }
    Some(AvatarInfo {
        icon: entry
            .get("avatar_icon")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        picture_file: entry
            .get("gaia_picture_file_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        uses_picture: entry
            .get("use_gaia_picture")
            .and_then(|v| v.as_bool())
            .unwrap_or_default(),
        is_default: entry
            .get("is_using_default_avatar")
            .and_then(|v| v.as_bool())
            .unwrap_or_default(),
    })
}

fn build_profile_snapshot(
    install: &BrowserInstall,
    directory: &str,
    entry: &serde_json::Value,
) -> BrowserProfile {
    let path = install.user_data_root.join(directory);
    let directory_exists = path.is_dir();
    let display_name = match entry
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(s) => s.to_string(),
        None => directory.to_string(),
    };

    let cache_path = install.cache_root.as_ref().and_then(|root| {
        let p = root.join(directory);
        if p.is_dir() {
            Some(p)
        } else {
            None
        }
    });

    let avatar = build_avatar_from_entry(entry);
    let last_active = entry
        .get("active_time")
        .and_then(|v| v.as_f64())
        .filter(|t| t.is_finite())
        .map(|t| t as i64);

    // Same normalization the snapshot builder applies: only a real value counts.
    let account_email = entry
        .get("user_name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    BrowserProfile {
        id: ProfileId::new(&install.id, directory),
        install_id: install.id.clone(),
        display_name,
        directory: directory.to_string(),
        path,
        cache_path,
        avatar,
        account_email,
        last_active,
        registered: true,
        directory_exists,
        size: None,
    }
}
