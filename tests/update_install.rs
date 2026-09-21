use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use profilemux::update::install::{classify_install, install_binary, InstallKind};

#[test]
fn install_binary_replaces_target_with_new_bytes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("pmux");
    std::fs::write(&target, b"old-binary-content").unwrap();

    let new_bytes = b"new-binary-content-v2";
    let replaced = install_binary(new_bytes, &target).unwrap();
    assert_eq!(replaced, target);

    let installed_content = std::fs::read(&target).unwrap();
    assert_eq!(installed_content, new_bytes);
}

#[test]
fn install_binary_preserves_executable_bit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("pmux");
    std::fs::write(&target, b"old-binary").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();

    let new_bytes = b"new-binary";
    install_binary(new_bytes, &target).unwrap();

    let perms = std::fs::metadata(&target).unwrap().permissions();
    assert_ne!(
        perms.mode() & 0o111,
        0,
        "executable bit should be preserved"
    );
}

#[test]
fn install_binary_read_only_directory_fails_and_preserves_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sub_dir = temp_dir.path().join("readonly_dir");
    std::fs::create_dir(&sub_dir).unwrap();

    let target = sub_dir.join("pmux");
    let original_bytes = b"original-unmodified-pmux";
    std::fs::write(&target, original_bytes).unwrap();

    // Make the directory read-only
    std::fs::set_permissions(&sub_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = install_binary(b"new-attempted-bytes", &target);
    assert!(
        result.is_err(),
        "install into read-only directory must fail"
    );

    // Restore permissions so tempdir cleanup can delete the directory
    let _ = std::fs::set_permissions(&sub_dir, std::fs::Permissions::from_mode(0o755));

    let content_after = std::fs::read(&target).unwrap();
    assert_eq!(
        content_after, original_bytes,
        "original file must be byte-identical"
    );
}

#[test]
fn install_binary_failure_leaves_original_intact_and_no_stray_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("pmux");
    let original_bytes = b"original-pmux-payload";
    std::fs::write(&target, original_bytes).unwrap();

    // Passing empty bytes triggers validation failure and rollback
    let result = install_binary(&[], &target);
    assert!(result.is_err(), "empty binary replacement must fail");

    // Verify original is intact
    let content = std::fs::read(&target).unwrap();
    assert_eq!(content, original_bytes, "original file must be intact");

    // Assert on directory listing: exactly only the target file remains
    let mut entries: Vec<String> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec!["pmux".to_string()],
        "directory must contain only the target file, with no stray tmp/backup files"
    );
}

#[test]
fn classify_install_scenarios() {
    assert_eq!(
        classify_install(Path::new("/x/target/release/pmux")),
        InstallKind::Development
    );
    assert_eq!(
        classify_install(Path::new("/x/target/debug/pmux")),
        InstallKind::Development
    );
    assert_eq!(
        classify_install(Path::new("/Users/user/.cargo/bin/pmux")),
        InstallKind::CargoBin
    );
    assert_eq!(
        classify_install(Path::new(".cargo/bin/pmux")),
        InstallKind::CargoBin
    );
    assert_eq!(
        classify_install(Path::new("/usr/local/bin/pmux")),
        InstallKind::Standalone
    );
    // User directory named release must not trigger Development
    assert_eq!(
        classify_install(Path::new("/Users/release/bin/pmux")),
        InstallKind::Standalone
    );
}
