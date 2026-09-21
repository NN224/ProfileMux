use std::path::{Path, PathBuf};
use tempfile::TempDir;

use profilemux::fs::trash::{send_to_dir, FakeTrash, TrashBin};
use profilemux::fs::Transaction;

fn snapshot_tree(root: &Path) -> std::io::Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut entries = Vec::new();
    collect_tree_entries(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn collect_tree_entries(
    base: &Path,
    current: &Path,
    out: &mut Vec<(PathBuf, Vec<u8>)>,
) -> std::io::Result<()> {
    if current.is_dir() {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let rel = match path.strip_prefix(base) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };
            if path.is_dir() {
                out.push((rel, Vec::new()));
                collect_tree_entries(base, &path, out)?;
            } else if path.is_file() {
                let content = std::fs::read(&path)?;
                out.push((rel, content));
            }
        }
    }
    Ok(())
}

#[test]
fn test_create_dir_rollback_leaves_parent() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let parent = tmp.path().join("existing_parent");
    std::fs::create_dir(&parent)?;
    let child = parent.join("nested").join("deep");

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.create_dir(&child)?;
    assert!(child.is_dir());

    tx.rollback()?;
    assert!(!child.exists());
    assert!(!parent.join("nested").exists());
    assert!(parent.is_dir());
    Ok(())
}

#[test]
fn test_write_file_existing_rollback_restores() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let file = tmp.path().join("target.txt");
    let original_bytes = b"original content";
    std::fs::write(&file, original_bytes)?;

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.write_file(&file, b"modified content")?;
    assert_eq!(std::fs::read(&file)?, b"modified content");

    tx.rollback()?;
    assert_eq!(std::fs::read(&file)?, original_bytes);
    Ok(())
}

#[test]
fn test_write_file_new_path_rollback_removes() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let file = tmp.path().join("new_file.txt");

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.write_file(&file, b"new content")?;
    assert!(file.is_file());

    tx.rollback()?;
    assert!(!file.exists());
    Ok(())
}

#[test]
fn test_copy_dir_rollback_removes_copied_tree() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let src = tmp.path().join("src_dir");
    let sub = src.join("sub");
    std::fs::create_dir_all(&sub)?;
    std::fs::write(src.join("a.txt"), b"file a")?;
    std::fs::write(sub.join("b.txt"), b"file b")?;

    let dst = tmp.path().join("dst_dir");

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.copy_dir(&src, &dst)?;
    assert!(dst.join("a.txt").is_file());
    assert!(dst.join("sub").join("b.txt").is_file());

    tx.rollback()?;
    assert!(!dst.exists());
    assert!(src.join("a.txt").is_file());
    assert!(sub.join("b.txt").is_file());
    Ok(())
}

#[test]
fn test_copy_file_existing_destination_fails() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let src = tmp.path().join("src.txt");
    let dst = tmp.path().join("dst.txt");
    std::fs::write(&src, b"source")?;
    std::fs::write(&dst, b"destination original")?;

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    let res = tx.copy_file(&src, &dst);
    assert!(res.is_err());
    assert_eq!(std::fs::read(&dst)?, b"destination original");
    Ok(())
}

#[test]
fn test_multistep_failure_rollback_preserves_tree() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let work = tmp.path().join("work");
    std::fs::create_dir_all(work.join("sub"))?;
    std::fs::write(work.join("file1.txt"), b"initial 1")?;
    std::fs::write(work.join("sub").join("file2.txt"), b"initial 2")?;

    let before = snapshot_tree(&work)?;

    let backup_dir = tmp.path().join("backups");
    let mut tx = Transaction::with_backup_root("test", &backup_dir)?;
    tx.create_dir(&work.join("new_dir"))?;
    tx.write_file(&work.join("file1.txt"), b"mutated 1")?;
    let step3 = tx.copy_file(&work.join("ghost.txt"), &work.join("dest.txt"));
    assert!(step3.is_err());

    tx.rollback()?;

    let after = snapshot_tree(&work)?;
    assert_eq!(before, after);
    Ok(())
}

#[test]
fn test_commit_keeps_changes_and_removes_backup_dir() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let work = tmp.path().join("work");
    std::fs::create_dir(&work)?;
    let file = work.join("committed.txt");

    let backup_dir = tmp.path().join("backups");
    let mut tx = Transaction::with_backup_root("test", &backup_dir)?;
    tx.write_file(&file, b"committed data")?;

    tx.commit()?;
    assert_eq!(std::fs::read(&file)?, b"committed data");

    let remaining_backups = if backup_dir.exists() {
        backup_dir.read_dir()?.count()
    } else {
        0
    };
    assert_eq!(remaining_backups, 0);
    Ok(())
}

#[test]
fn test_drop_without_commit_or_rollback_undoes_changes() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let file = tmp.path().join("drop_test.txt");

    {
        let mut tx = Transaction::with_backup_root("test", tmp.path())?;
        tx.write_file(&file, b"will be dropped")?;
        assert!(file.is_file());
    }

    assert!(!file.exists());
    Ok(())
}

#[test]
fn test_send_to_dir_moves_directory_to_trash() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let trash_dir = tmp.path().join("Trash");
    let fake_trash = FakeTrash::new(&trash_dir);

    let profile_dir = tmp.path().join("Profile 1");
    std::fs::create_dir(&profile_dir)?;
    std::fs::write(profile_dir.join("Preferences"), b"{\"pref\": 1}")?;

    let dest = fake_trash.send(&profile_dir)?;
    assert!(!profile_dir.exists());
    assert!(dest.is_dir());
    assert!(dest.join("Preferences").is_file());
    assert_eq!(std::fs::read(dest.join("Preferences"))?, b"{\"pref\": 1}");
    Ok(())
}

#[test]
fn test_send_to_dir_twice_creates_distinct_entries() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let trash_dir = tmp.path().join("Trash");

    let p1 = tmp.path().join("Profile 1");
    std::fs::create_dir(&p1)?;
    std::fs::write(p1.join("data.txt"), b"first profile")?;

    let dest1 = send_to_dir(&trash_dir, &p1)?;

    let p2 = tmp.path().join("Profile 1");
    std::fs::create_dir(&p2)?;
    std::fs::write(p2.join("data.txt"), b"second profile")?;

    let dest2 = send_to_dir(&trash_dir, &p2)?;

    assert_ne!(dest1, dest2);
    assert_eq!(dest1.file_name().unwrap(), "Profile 1");
    assert_eq!(dest2.file_name().unwrap(), "Profile 1 2");
    assert_eq!(std::fs::read(dest1.join("data.txt"))?, b"first profile");
    assert_eq!(std::fs::read(dest2.join("data.txt"))?, b"second profile");
    Ok(())
}

#[test]
fn test_backup_file_rollback_restores() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let file = tmp.path().join("local_state.json");
    std::fs::write(&file, b"{\"initial\": true}")?;

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.backup_file(&file)?;
    std::fs::write(&file, b"{\"mutated\": true}")?;

    tx.rollback()?;
    assert_eq!(std::fs::read(&file)?, b"{\"initial\": true}");
    Ok(())
}

#[test]
fn test_rename_rollback_restores() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let from = tmp.path().join("old_name");
    let to = tmp.path().join("new_name");
    std::fs::write(&from, b"rename test")?;

    let mut tx = Transaction::with_backup_root("test", tmp.path())?;
    tx.rename(&from, &to)?;
    assert!(!from.exists());
    assert!(to.exists());

    tx.rollback()?;
    assert!(from.exists());
    assert!(!to.exists());
    assert_eq!(std::fs::read(&from)?, b"rename test");
    Ok(())
}

#[test]
fn test_send_to_dir_not_found() {
    let tmp = TempDir::new().unwrap();
    let trash_dir = tmp.path().join("Trash");
    let missing = tmp.path().join("does_not_exist");

    let err = send_to_dir(&trash_dir, &missing);
    assert!(matches!(err, Err(profilemux::Error::NotFound(_))));
}
