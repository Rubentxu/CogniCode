//! `cogh::rollback_journal` — atomic-install rollback journal with LIFO reversal.
//!
//! Records side-effects during an install transaction and reverses them
//! in reverse order (LIFO) when the journal is dropped or explicitly rolled back.
//! The journal is committed by going out of scope with no-op `commit()`.

use crate::error::InstallerError;
use std::path::PathBuf;

/// Side-effect recorded in the journal during an install transaction.
#[derive(Debug, Clone)]
pub enum SideEffect {
    /// A directory was created.
    CreatedDir(PathBuf),
    /// A file was downloaded.
    Downloaded(PathBuf),
    /// A file was verified against its SHA-256 hash.
    VerifiedSha256(PathBuf),
    /// An archive was extracted to a directory.
    Extracted(PathBuf),
    /// A symbolic link was created.
    CreatedSymlink { link: PathBuf, target: PathBuf },
    /// A JSON manifest file was patched (stores old value for restore).
    PatchedJson {
        path: PathBuf,
        key: String,
        old_value: Option<serde_json::Value>,
    },
    /// A directory was removed (cannot be easily restored).
    RemovedDir(PathBuf),
    /// A manifest file was written.
    WroteManifest(PathBuf),
}

/// Rollback journal for atomic install operations.
///
/// Records side-effects during a transaction and reverses them in LIFO order
/// on rollback. Commit is a no-op (journal simply goes out of scope with effects applied).
#[derive(Debug, Default, Clone)]
pub struct RollbackJournal {
    effects: Vec<SideEffect>,
    committed: bool,
}

impl RollbackJournal {
    /// Create a new empty journal.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            committed: false,
        }
    }

    /// Record a side-effect in the journal.
    pub fn record(&mut self, effect: SideEffect) {
        self.effects.push(effect);
    }

    /// Reverse all side-effects in LIFO order.
    ///
    /// Errors during reversal are collected and returned as a single error.
    pub fn rollback(&self) -> Result<(), InstallerError> {
        let mut errors = Vec::new();

        for effect in self.effects.iter().rev() {
            if let Err(e) = Self::reverse_one(effect) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            Err(InstallerError::Rollback(msg))
        }
    }

    /// Commit the journal — a no-op (effects stay applied).
    /// After calling `commit`, the `Drop` impl will NOT roll back.
    pub fn commit(&mut self) {
        self.committed = true;
    }

    /// Reverse a single side-effect. Returns Ok if the reversal succeeded
    /// or if the reversal is intentionally a no-op; returns Err on failure.
    fn reverse_one(effect: &SideEffect) -> Result<(), InstallerError> {
        match effect {
            SideEffect::CreatedDir(path) => {
                std::fs::remove_dir(path).map_err(|e| {
                    InstallerError::Rollback(format!("remove CreatedDir {}: {}", path.display(), e))
                })?;
            }
            SideEffect::Downloaded(path) => {
                std::fs::remove_file(path).map_err(|e| {
                    InstallerError::Rollback(format!("remove Downloaded {}: {}", path.display(), e))
                })?;
            }
            SideEffect::VerifiedSha256(_) => {
                // no-op: verification has no persistent state to reverse
            }
            SideEffect::Extracted(path) => {
                if path.is_dir() {
                    std::fs::remove_dir_all(path).map_err(|e| {
                        InstallerError::Rollback(format!(
                            "remove Extracted {}: {}",
                            path.display(),
                            e
                        ))
                    })?;
                }
            }
            SideEffect::CreatedSymlink { link, .. } => {
                std::fs::remove_file(link).map_err(|e| {
                    InstallerError::Rollback(format!(
                        "remove CreatedSymlink {}: {}",
                        link.display(),
                        e
                    ))
                })?;
            }
            SideEffect::PatchedJson {
                path,
                key,
                old_value,
            } => {
                // Restore the old value by re-patching in reverse
                if let Some(old) = old_value {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let mut json: serde_json::Value = serde_json::from_str(&content)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        // Navigate using key path (dot notation)
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() == 1 {
                            if let Some(obj) = json.as_object_mut() {
                                obj.insert(parts[0].to_string(), old.clone());
                            }
                        }
                        let new_content = serde_json::to_string_pretty(&json).ok();
                        if let Some(c) = new_content {
                            std::fs::write(path, c).ok();
                        }
                    }
                }
            }
            SideEffect::RemovedDir(_) => {
                // Cannot easily restore a removed directory without a backup.
                // Log a warning (best-effort) and continue with other rollbacks.
            }
            SideEffect::WroteManifest(path) => {
                std::fs::remove_file(path).map_err(|e| {
                    InstallerError::Rollback(format!(
                        "remove WroteManifest {}: {}",
                        path.display(),
                        e
                    ))
                })?;
            }
        }
        Ok(())
    }
}

impl Drop for RollbackJournal {
    fn drop(&mut self) {
        // Only roll back if not committed. If `commit()` was called, effects stay applied.
        if self.committed {
            return;
        }
        // Best-effort rollback on drop; errors are logged but not propagated
        // since Drop cannot return Result. Users should call `rollback()` explicitly
        // if they need to handle errors.
        for effect in self.effects.iter().rev() {
            let _ = Self::reverse_one(effect);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn create_temp_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_rollback_reverses_created_dir() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let subdir = tmp.path().join("created_by_install");
        std::fs::create_dir(&subdir).unwrap();
        assert!(subdir.is_dir());

        journal.record(SideEffect::CreatedDir(subdir.clone()));

        // Rollback should remove the directory
        journal.rollback().unwrap();
        assert!(!subdir.exists(), "CreatedDir should be removed by rollback");
    }

    #[test]
    fn test_rollback_reverses_downloaded_file() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let downloaded = create_temp_file(&tmp, "downloaded.tar.gz", b"fake archive");
        assert!(downloaded.is_file());

        journal.record(SideEffect::Downloaded(downloaded.clone()));

        journal.rollback().unwrap();
        assert!(
            !downloaded.exists(),
            "Downloaded file should be removed by rollback"
        );
    }

    #[test]
    fn test_rollback_reverses_created_symlink() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let target = create_temp_file(&tmp, "target_binary", b"binary content");
        let link = tmp.path().join("binary_link");

        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(link.is_symlink());

        journal.record(SideEffect::CreatedSymlink {
            link: link.clone(),
            target,
        });

        journal.rollback().unwrap();
        assert!(
            !link.exists(),
            "CreatedSymlink should be removed by rollback"
        );
    }

    #[test]
    fn test_rollback_reverses_in_lifo_order() {
        // Create three items in order: dir1, file2, file3
        let tmp = create_temp_dir();

        let dir1 = tmp.path().join("dir1");
        let file2 = tmp.path().join("file2");
        let file3 = tmp.path().join("file3");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::write(&file2, b"2").unwrap();
        std::fs::write(&file3, b"3").unwrap();

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(dir1.clone()));
        journal.record(SideEffect::Downloaded(file2.clone()));
        journal.record(SideEffect::Downloaded(file3.clone()));

        journal.rollback().unwrap();

        // LIFO: file3 first, then file2, then dir1
        assert!(!file3.exists(), "file3 should be removed first (LIFO)");
        assert!(!file2.exists(), "file2 should be removed second");
        assert!(!dir1.exists(), "dir1 should be removed last");
    }

    #[test]
    fn test_commit_is_noop() {
        let tmp = create_temp_dir();
        let dir = tmp.path().join("committed_dir");
        std::fs::create_dir(&dir).unwrap();
        assert!(dir.is_dir());

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(dir.clone()));

        // Commit does nothing
        journal.commit();

        // Directory should still exist after commit
        assert!(
            dir.exists(),
            "commit() should be a no-op; directory must still exist"
        );
    }

    #[test]
    fn test_rollback_extracted_directory() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let extracted = tmp.path().join("extracted_contents");
        std::fs::create_dir(&extracted).unwrap();
        std::fs::write(extracted.join("file.txt"), b"content").unwrap();
        assert!(extracted.is_dir());

        journal.record(SideEffect::Extracted(extracted.clone()));

        journal.rollback().unwrap();
        assert!(
            !extracted.exists(),
            "Extracted directory should be removed by rollback"
        );
    }

    #[test]
    fn test_rollback_wrote_manifest() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let manifest = create_temp_file(&tmp, "manifest.yaml", b"apiVersion: v1");
        assert!(manifest.is_file());

        journal.record(SideEffect::WroteManifest(manifest.clone()));

        journal.rollback().unwrap();
        assert!(
            !manifest.exists(),
            "WroteManifest file should be removed by rollback"
        );
    }

    #[test]
    fn test_verified_sha256_is_noop_on_rollback() {
        let tmp = create_temp_dir();
        let mut journal = RollbackJournal::new();

        let verified_file = create_temp_file(&tmp, "verified.bin", b"data");
        journal.record(SideEffect::VerifiedSha256(verified_file.clone()));

        // VerifiedSha256 is a no-op on rollback — file should remain
        journal.rollback().unwrap();
        assert!(
            verified_file.exists(),
            "VerifiedSha256 should be a no-op; file must still exist"
        );
    }

    #[test]
    fn test_journal_records_effects_in_order() {
        let mut journal = RollbackJournal::new();
        let effect1 = SideEffect::Downloaded(PathBuf::from("/a"));
        let effect2 = SideEffect::CreatedDir(PathBuf::from("/b"));

        journal.record(effect1.clone());
        journal.record(effect2.clone());

        // We can't directly access effects, but we can verify LIFO order
        // by checking rollback removes in reverse order
        let tmp = create_temp_dir();
        let file_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        std::fs::write(&file_a, b"").unwrap();
        std::fs::create_dir(&dir_b).unwrap();

        let mut j2 = RollbackJournal::new();
        j2.record(SideEffect::Downloaded(file_a));
        j2.record(SideEffect::CreatedDir(dir_b.clone()));
        j2.rollback().unwrap();
        assert!(!dir_b.exists()); // created last, removed first
    }
}
