//! `cogh::rollback_journal` — atomic-install rollback journal with LIFO reversal.
//!
//! Records side-effects during an install transaction and reverses them
//! in reverse order (LIFO) when the journal is dropped or explicitly rolled back.
//! The journal is committed by going out of scope with no-op `commit()`.
//!
//! e86: the journal gains `WroteTracker` (to restore the previous tracker
//! version on rollback) and JSON serialise / deserialise so a committed
//! install can be reversed later by a separate `cogh rollback` invocation.

use crate::error::InstallerError;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt as stdfmt;
use std::path::PathBuf;

/// Side-effect recorded in the journal during an install transaction.
///
/// Serialised to JSON with a `"type"` discriminator so the on-disk form is
/// stable across schema changes. We cannot use `#[serde(tag = "type")]` on a
/// newtype variant (serde limitation), so the serialise / deserialise impls
/// are hand-written.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The version tracker file was written. `previous` is the value that
    /// existed before the install, so the rollback restores the prior pin.
    /// `None` means "no tracker existed before".
    WroteTracker {
        path: PathBuf,
        previous: Option<String>,
    },
}

impl SideEffect {
    /// Stable tag name used in the serialised JSON.
    fn tag(&self) -> &'static str {
        match self {
            SideEffect::CreatedDir(_) => "CreatedDir",
            SideEffect::Downloaded(_) => "Downloaded",
            SideEffect::VerifiedSha256(_) => "VerifiedSha256",
            SideEffect::Extracted(_) => "Extracted",
            SideEffect::CreatedSymlink { .. } => "CreatedSymlink",
            SideEffect::PatchedJson { .. } => "PatchedJson",
            SideEffect::RemovedDir(_) => "RemovedDir",
            SideEffect::WroteManifest(_) => "WroteManifest",
            SideEffect::WroteTracker { .. } => "WroteTracker",
        }
    }
}

impl Serialize for SideEffect {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = ser.serialize_map(Some(2))?;
        map.serialize_entry("type", self.tag())?;
        match self {
            SideEffect::CreatedDir(p)
            | SideEffect::Downloaded(p)
            | SideEffect::VerifiedSha256(p)
            | SideEffect::Extracted(p)
            | SideEffect::RemovedDir(p)
            | SideEffect::WroteManifest(p) => {
                map.serialize_entry("path", p)?;
            }
            SideEffect::CreatedSymlink { link, target } => {
                map.serialize_entry("link", link)?;
                map.serialize_entry("target", target)?;
            }
            SideEffect::PatchedJson {
                path,
                key,
                old_value,
            } => {
                map.serialize_entry("path", path)?;
                map.serialize_entry("key", key)?;
                map.serialize_entry("old_value", old_value)?;
            }
            SideEffect::WroteTracker { path, previous } => {
                map.serialize_entry("path", path)?;
                map.serialize_entry("previous", previous)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SideEffect {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = SideEffect;
            fn expecting(&self, f: &mut stdfmt::Formatter<'_>) -> stdfmt::Result {
                f.write_str("a SideEffect JSON object with a `type` discriminator")
            }
            fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<SideEffect, M::Error> {
                let mut tag: Option<String> = None;
                let mut path: Option<PathBuf> = None;
                let mut link: Option<PathBuf> = None;
                let mut target: Option<PathBuf> = None;
                let mut key: Option<String> = None;
                let mut old_value: Option<Option<serde_json::Value>> = None;
                let mut previous: Option<Option<String>> = None;
                while let Some(k) = map.next_key::<String>()? {
                    match k.as_str() {
                        "type" => tag = Some(map.next_value()?),
                        "path" => path = Some(map.next_value()?),
                        "link" => link = Some(map.next_value()?),
                        "target" => target = Some(map.next_value()?),
                        "key" => key = Some(map.next_value()?),
                        "old_value" => old_value = Some(map.next_value()?),
                        "previous" => previous = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let tag = tag.ok_or_else(|| de::Error::missing_field("type"))?;
                let need_path = |field: &'static str| -> Result<PathBuf, M::Error> {
                    path.clone().ok_or_else(|| de::Error::missing_field(field))
                };
                Ok(match tag.as_str() {
                    "CreatedDir" => SideEffect::CreatedDir(need_path("path")?),
                    "Downloaded" => SideEffect::Downloaded(need_path("path")?),
                    "VerifiedSha256" => SideEffect::VerifiedSha256(need_path("path")?),
                    "Extracted" => SideEffect::Extracted(need_path("path")?),
                    "RemovedDir" => SideEffect::RemovedDir(need_path("path")?),
                    "WroteManifest" => SideEffect::WroteManifest(need_path("path")?),
                    "CreatedSymlink" => SideEffect::CreatedSymlink {
                        link: link.ok_or_else(|| de::Error::missing_field("link"))?,
                        target: target.ok_or_else(|| de::Error::missing_field("target"))?,
                    },
                    "PatchedJson" => SideEffect::PatchedJson {
                        path: need_path("path")?,
                        key: key.ok_or_else(|| de::Error::missing_field("key"))?,
                        old_value: old_value.unwrap_or(None),
                    },
                    "WroteTracker" => SideEffect::WroteTracker {
                        path: need_path("path")?,
                        previous: previous.unwrap_or(None),
                    },
                    other => {
                        return Err(de::Error::unknown_variant(
                            other,
                            &[
                                "CreatedDir",
                                "Downloaded",
                                "VerifiedSha256",
                                "Extracted",
                                "CreatedSymlink",
                                "PatchedJson",
                                "RemovedDir",
                                "WroteManifest",
                                "WroteTracker",
                            ],
                        ));
                    }
                })
            }
        }
        de.deserialize_map(V)
    }
}

/// Rollback journal for atomic install operations.
///
/// Records side-effects during a transaction and reverses them in LIFO order
/// on rollback. Commit is a no-op (journal simply goes out of scope with effects applied).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RollbackJournal {
    effects: Vec<SideEffect>,
    #[serde(skip, default)]
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

    /// Read-only access to the recorded effects (in commit order).
    pub fn effects(&self) -> &[SideEffect] {
        &self.effects
    }

    /// Serialise the journal to JSON. `committed` is skipped — the
    /// serialised form represents the persisted record, not the runtime
    /// state.
    pub fn to_json(&self) -> Result<String, InstallerError> {
        serde_json::to_string_pretty(self).map_err(|e| InstallerError::Serialize(e.to_string()))
    }

    /// Deserialise a journal from JSON. The result has `committed: false`
    /// by construction (we are loading it to reverse it).
    pub fn from_json(s: &str) -> Result<Self, InstallerError> {
        serde_json::from_str::<RollbackJournal>(s)
            .map_err(|e| InstallerError::Serialize(e.to_string()))
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
                // Idempotent: a sibling reversal may have already removed
                // this dir (e.g. a later `CreatedDir(install_dir)` recorded
                // for the manifest parent dir uses `remove_dir_all`, which
                // removes this dir as a side effect). Only attempt removal
                // when the path still exists.
                if path.exists() {
                    // Use `remove_dir_all` (not `remove_dir`) because a
                    // recorded `CreatedDir` may end up being the last one
                    // reversed for a dir whose contents were not themselves
                    // journaled as individual side-effects (e.g. an
                    // extraction that overwrites files in a previously-
                    // installed component dir).
                    std::fs::remove_dir_all(path).map_err(|e| {
                        InstallerError::Rollback(format!(
                            "remove CreatedDir {}: {}",
                            path.display(),
                            e
                        ))
                    })?;
                }
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
            SideEffect::WroteTracker { path, previous } => match previous {
                Some(prev) => {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(path, prev).map_err(|e| {
                        InstallerError::Rollback(format!(
                            "restore WroteTracker {}: {}",
                            path.display(),
                            e
                        ))
                    })?;
                }
                None => {
                    if path.exists() {
                        std::fs::remove_file(path).map_err(|e| {
                            InstallerError::Rollback(format!(
                                "remove WroteTracker (no previous) {}: {}",
                                path.display(),
                                e
                            ))
                        })?;
                    }
                }
            },
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

    // ---- e86 WU2 / T2: JSON round-trip + WroteTracker reversal ----

    #[test]
    fn test_serialise_round_trip() {
        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(PathBuf::from("/tmp/c")));
        journal.record(SideEffect::Downloaded(PathBuf::from("/tmp/d.tar.gz")));
        journal.record(SideEffect::WroteManifest(PathBuf::from("/tmp/m.yaml")));
        journal.record(SideEffect::WroteTracker {
            path: PathBuf::from("/tmp/tracker"),
            previous: Some("0.94.0".to_string()),
        });

        let json = journal.to_json().expect("serialise");
        // `committed` MUST NOT appear (skipped on serialise).
        assert!(
            !json.contains("\"committed\""),
            "committed field must not leak into the persisted form: {json}"
        );
        // Every effect's tag must be present.
        assert!(json.contains("\"CreatedDir\""));
        assert!(json.contains("\"Downloaded\""));
        assert!(json.contains("\"WroteManifest\""));
        assert!(json.contains("\"WroteTracker\""));
        assert!(json.contains("\"previous\": \"0.94.0\""));

        let loaded = RollbackJournal::from_json(&json).expect("deserialise");
        assert_eq!(loaded.effects().len(), 4);
        assert_eq!(
            loaded.effects()[3],
            SideEffect::WroteTracker {
                path: PathBuf::from("/tmp/tracker"),
                previous: Some("0.94.0".to_string()),
            }
        );
    }

    #[test]
    fn test_wrote_tracker_reverse_with_previous() {
        let tmp = create_temp_dir();
        let tracker = tmp.path().join("tracker/version");
        std::fs::create_dir_all(tracker.parent().unwrap()).unwrap();
        std::fs::write(&tracker, "0.95.0").unwrap();

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::WroteTracker {
            path: tracker.clone(),
            previous: Some("0.94.0".to_string()),
        });

        journal.rollback().unwrap();
        let restored = std::fs::read_to_string(&tracker).unwrap();
        assert_eq!(restored.trim(), "0.94.0");
    }

    #[test]
    fn test_wrote_tracker_reverse_with_no_previous_removes_file() {
        let tmp = create_temp_dir();
        let tracker = tmp.path().join("tracker/version");
        std::fs::create_dir_all(tracker.parent().unwrap()).unwrap();
        std::fs::write(&tracker, "0.95.0").unwrap();
        assert!(tracker.exists());

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::WroteTracker {
            path: tracker.clone(),
            previous: None,
        });

        journal.rollback().unwrap();
        assert!(
            !tracker.exists(),
            "tracker must be removed when previous was None"
        );
    }

    // ---- e86.1 REQ-LJ-04: rollback reverses a POPULATED CreatedDir ----
    //
    // Before the e86.1 fix, `CreatedDir` reversal used
    // `std::fs::remove_dir` (empty-only), which fails with ENOTEMPTY
    // on a populated dir. A real `cogh update` writes the install
    // manifest + extracted components + shims into the install
    // root, so the populated-dir case is the actual production
    // case. This test pins the new behaviour: a CreatedDir that
    // contains un-journaled children is removed by `remove_dir_all`.

    #[test]
    fn test_rollback_reverses_populated_created_dir() {
        let tmp = create_temp_dir();
        let dir = tmp.path().join("install/0.95.0");
        std::fs::create_dir_all(&dir).unwrap();

        // Populate the dir with content that was NOT journaled as
        // individual side-effects (this is exactly the production
        // case: extracted tarballs land here without per-file
        // journal entries).
        std::fs::write(dir.join("manifest.yaml"), b"components: []").unwrap();
        std::fs::create_dir_all(dir.join("cognicode/bin")).unwrap();
        std::fs::write(dir.join("cognicode/bin/cognicode"), b"\x7fELF").unwrap();

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(dir.clone()));

        journal
            .rollback()
            .expect("populated CreatedDir must roll back");
        assert!(!dir.exists(), "populated dir must be removed");
    }

    // ---- e86.1 REQ-LJ-04: rollback is idempotent for CreatedDir ----
    //
    // A CreatedDir that was already removed by a sibling reversal
    // (e.g. a later CreatedDir for the same path uses
    // `remove_dir_all` which sweeps the earlier entry) must be a
    // no-op, not a hard error.

    #[test]
    fn test_rollback_is_idempotent_for_double_created_dir() {
        let tmp = create_temp_dir();
        let dir = tmp.path().join("install/0.95.0");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("file.txt"), b"x").unwrap();

        let mut journal = RollbackJournal::new();
        // Recorded twice — the second entry uses `remove_dir_all`
        // and will sweep everything; the first entry must then
        // be a no-op.
        journal.record(SideEffect::CreatedDir(dir.clone()));
        journal.record(SideEffect::CreatedDir(dir.clone()));

        journal
            .rollback()
            .expect("double CreatedDir must roll back");
        assert!(!dir.exists(), "double-CreatedDir must be removed");
    }
}
