//! `cogh::lifecycle_journal` — Persistent rollback journal for `cogh rollback`.
//!
//! e86: when an install commits, the journal is written next to the install
//! so a later `cogh rollback` command can reverse it. The on-disk shape is
//! the same [`RollbackJournal::to_json`] form, plus an envelope with the
//! version and committed-at timestamp.
//!
//! ```text
//! ~/.cognicode/journal/<version>.json
//! {
//!   "version": "0.95.0",
//!   "committed_at_unix": 1734567890,
//!   "previous_tracker": "0.94.0",     // null if no prior install
//!   "effects": [ SideEffect, ... ]
//! }
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bundle_manifest::BundleManifest;
use crate::error::InstallerError;
use crate::layout::cognicode_home;
use crate::rollback_journal::{RollbackJournal, SideEffect};

/// On-disk envelope for a committed install's journal.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedJournal {
    /// Version of the install this journal describes.
    pub version: String,
    /// Unix epoch seconds at commit time. Optional for forward-compat.
    #[serde(default)]
    pub committed_at_unix: Option<u64>,
    /// The value the tracker held before this install (so rollback can
    /// restore it). `None` if no tracker existed before.
    #[serde(default)]
    pub previous_tracker: Option<String>,
    /// The journal itself, in commit order (rollback reverses LIFO).
    pub effects: RollbackJournal,
}

/// Path to the journal for a given version.
///
/// DEPRECATED for production use: resolves via `COGNICODE_HOME`/`HOME`
/// and ignores `--home`. Callers holding a [`crate::layout::CognicodeHome`]
/// MUST use `home.journal_version(version)` instead (H-F6-1: a single
/// resolution of home).
pub fn journal_path(version: &str) -> PathBuf {
    cognicode_home()
        .join("journal")
        .join(format!("{version}.json"))
}

/// Persist the journal of a committed install.
///
/// `journal` carries the side-effects in commit order. The on-disk form
/// embeds them inside the envelope so a future `cogh rollback` can replay
/// the reversal with no other context.
///
/// **Atomicity (U21 / DEFECT-2026-09-23-A):** the write uses
/// temp-file + `fs::rename` so a SIGKILL between `create_dir_all` and
/// the syscall cannot leave a truncated payload at the canonical path.
/// Readers either see the prior envelope or the new envelope, never a
/// partial file.  This matches the pattern in
/// `application::services::file_operations::write_file`.
pub fn write(
    journal: &RollbackJournal,
    manifest: &BundleManifest,
    previous_tracker: Option<&str>,
    path: &Path,
) -> Result<(), InstallerError> {
    // We need a copy because the persisted form embeds the journal inside
    // the envelope, but `RollbackJournal` has a `Drop` that rolls back
    // uncommitted side-effects. The clone of an uncommitted journal would
    // execute that rollback when the local goes out of scope — including
    // reversing the `WroteManifest` we just wrote. We mark the clone as
    // committed so its Drop is a no-op.
    let mut persisted = journal.clone();
    persisted.commit();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| InstallerError::Io(parent.into(), e))?;
    }
    let committed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .ok();
    let envelope = PersistedJournal {
        version: manifest.version.clone(),
        committed_at_unix,
        previous_tracker: previous_tracker.map(|s| s.to_string()),
        effects: persisted,
    };
    let text = serde_json::to_string_pretty(&envelope)
        .map_err(|e| InstallerError::Serialize(e.to_string()))?;

    // Atomic write: temp file + rename.  The rename is atomic on POSIX
    // (rename(2)) and on NTFS within the same volume, so a concurrent
    // reader — or a SIGKILL mid-write — sees either the prior payload
    // or the new one, never a truncated file.  On failure, the temp
    // file is best-effort removed so it does not pollute the journal
    // directory.
    let temp_path = format!("{}.tmp.{}", path.display(), std::process::id());
    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| InstallerError::Io(std::path::PathBuf::from(&temp_path), e))?;
        std::io::Write::write_all(&mut file, text.as_bytes())
            .map_err(|e| InstallerError::Io(std::path::PathBuf::from(&temp_path), e))?;
        let _ = file.sync_all();
    }
    if let Err(e) = std::fs::rename(&temp_path, path) {
        // Best-effort cleanup of the temp file; the rename failure is
        // the primary error returned to the caller.
        let _ = std::fs::remove_file(&temp_path);
        return Err(InstallerError::Io(path.into(), e));
    }
    Ok(())
}

/// Load a journal from disk and return its in-memory form.
///
/// DEBT-4 architectural rule: a deserialized RollbackJournal MUST NOT
/// retain armed Drop rollback behaviour. The returned journal is
/// Drop-neutralized (`committed = true`): dropping it never replays a
/// reversal. Executing a rollback is an explicit `journal.rollback()`
/// call by a consumer that has validated applicability (tracker match).
pub fn load(path: &Path) -> Result<RollbackJournal, InstallerError> {
    let text = std::fs::read_to_string(path).map_err(|e| InstallerError::Io(path.into(), e))?;
    let mut envelope: PersistedJournal = serde_json::from_str(&text)
        .map_err(|e| InstallerError::Serialize(format!("journal `{}`: {}", path.display(), e)))?;
    envelope.effects.commit();
    Ok(envelope.effects)
}

/// Load the full envelope (so callers can see the previous tracker value).
pub fn load_envelope(path: &Path) -> Result<PersistedJournal, InstallerError> {
    let text = std::fs::read_to_string(path).map_err(|e| InstallerError::Io(path.into(), e))?;
    serde_json::from_str(&text)
        .map_err(|e| InstallerError::Serialize(format!("journal `{}`: {}", path.display(), e)))
}

/// Minimal envelope metadata for inspection: only `version` and
/// `previous_tracker`. **Crucially, this does NOT construct a
/// `RollbackJournal`, so it has NO destructive Drop side-effects.**
///
/// Use this when you only need to decide which envelope is the pending
/// rollback (by looking at `previous_tracker`) or to display the
/// install version. Do NOT use this when you actually want to replay
/// the reversal — for that, use [`load_envelope`] or [`load`] and
/// commit/cancel explicitly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeMetadata {
    pub version: String,
    #[serde(default)]
    pub previous_tracker: Option<String>,
}

pub fn peek_envelope_metadata(path: &Path) -> Result<EnvelopeMetadata, InstallerError> {
    let text = std::fs::read_to_string(path).map_err(|e| InstallerError::Io(path.into(), e))?;
    serde_json::from_str(&text)
        .map_err(|e| InstallerError::Serialize(format!("journal `{}`: {}", path.display(), e)))
}

/// Remove a journal after a successful rollback. Best-effort; failures
/// are silent because the install has already been reversed and there is
/// no useful recovery to do here.
pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle_manifest::BundleManifest;
    use serial_test::serial;
    use tempfile::TempDir;

    /// DEBT-4 tripwire: every deserialized journal MUST be Drop-neutralized.
    /// Dropping a loaded journal in an early-return path must never silently
    /// replay a reversal (the journal is an observation until the consumer
    /// explicitly validates applicability and calls `rollback()`).
    #[test]
    #[serial]
    fn t_debt4_loaded_journal_is_drop_neutralized() {
        let tmp = TempDir::new().unwrap();
        let journal_dir = tmp.path().join("journal");
        std::fs::create_dir_all(&journal_dir).unwrap();
        let marker_dir = tmp.path().join("versions").join("0.95.0");
        std::fs::create_dir_all(&marker_dir).unwrap();
        let manifest = marker_dir.join("manifest.yaml");
        std::fs::write(&manifest, "version: 0.95.0\n").unwrap();

        unsafe {
            std::env::set_var("COGNICODE_HOME", tmp.path());
        }

        let mut j = crate::rollback_journal::RollbackJournal::new();
        j.record(crate::rollback_journal::SideEffect::WroteManifest(
            manifest.clone(),
        ));
        let manifest_fixture = manifest_for("0.95.0");
        write(
            &j,
            &manifest_fixture,
            None,
            &journal_dir.join("0.95.0.json"),
        )
        .unwrap();

        let path = journal_path("0.95.0");
        // Drop without commit: the loaded journal must NOT revert effects.
        {
            let loaded = load(&path).expect("load must succeed");
            let _ = loaded; // dropped armed = bug
        }
        assert!(
            manifest.exists(),
            "DEBT-4: a deserialized journal must be Drop-neutralized; \
             dropping it must not silently replay a reversal"
        );
        // Same guarantee through from_json (source-level neutralization).
        {
            let loaded2 =
                crate::rollback_journal::RollbackJournal::from_json(&j.to_json().unwrap())
                    .expect("from_json must succeed");
            let _ = loaded2;
        }
        assert!(
            manifest.exists(),
            "DEBT-4: from_json must also produce a Drop-neutralized journal"
        );

        unsafe {
            if let Ok(v) = std::env::var("COGNICODE_HOME_PREV") {
                std::env::set_var("COGNICODE_HOME", v);
            } else {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
    }

    fn manifest_for(version: &str) -> BundleManifest {
        let yaml = format!(
            r#"
apiVersion: cognicode.bundle/v2
version: "{version}"
platform: linux-x86-64
profiles:
  - name: core
    description: core
components:
  - name: cognicode
    kind: cognicode
    version: "{version}"
    artifact: cognicode-{version}-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/cognicode-{version}-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#
        );
        BundleManifest::from_str(&yaml).expect("manifest must parse as v2")
    }

    #[test]
    fn test_write_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("0.95.0.json");
        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(tmp.path().join("dir")));
        journal.record(SideEffect::Downloaded(tmp.path().join("c.tar.gz")));
        journal.record(SideEffect::WroteManifest(tmp.path().join("manifest.yaml")));
        journal.record(SideEffect::WroteTracker {
            path: tmp.path().join("tracker/version"),
            previous: Some("0.94.0".to_string()),
        });

        let manifest = manifest_for("0.95.0");
        write(&journal, &manifest, Some("0.94.0"), &path).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.effects().len(), 4);
        assert_eq!(
            loaded.effects()[3],
            SideEffect::WroteTracker {
                path: tmp.path().join("tracker/version"),
                previous: Some("0.94.0".to_string()),
            }
        );

        let envelope = load_envelope(&path).unwrap();
        assert_eq!(envelope.version, "0.95.0");
        assert_eq!(envelope.previous_tracker.as_deref(), Some("0.94.0"));
    }

    #[test]
    fn test_load_corrupt_json_fails_loudly() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("corrupt.json");
        std::fs::write(&path, "{ this is not json").unwrap();

        let err = load(&path).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("journal") && msg.contains("corrupt"),
            "error must name the file: {msg}"
        );
    }

    #[test]
    fn test_persisted_envelope_with_no_previous_tracker() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("0.95.0.json");
        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(tmp.path().join("d")));
        let manifest = manifest_for("0.95.0");
        write(&journal, &manifest, None, &path).unwrap();
        let envelope = load_envelope(&path).unwrap();
        assert_eq!(envelope.previous_tracker, None);
    }

    /// U21 / DEFECT-2026-09-23-A: `write` MUST be atomic so a SIGKILL
    /// mid-write cannot leave a truncated journal at the canonical path.
    /// Concretely: after a successful `write`, no `.tmp*` artifact may
    /// remain in the parent directory; the canonical path either exists
    /// (with a complete payload) or does not exist (no partial state).
    ///
    /// The production write currently uses a non-atomic
    /// `std::fs::write(path, text)` (see `write` impl above), which
    /// violates U21 under crash between create_dir_all and the write
    /// syscall.  The GREEN fix is to use the same temp+rename pattern
    /// already adopted by `file_operations::write_file`:
    ///   1. Write text to `<path>.tmp.<pid>`
    ///   2. `fs::rename` to canonical path (atomic on POSIX/NTFS)
    ///
    /// This test pins the contract that the fix MUST satisfy.
    #[test]
    fn test_write_is_atomic_no_tmp_artifact_left_on_success() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("0.95.0.json");
        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(tmp.path().join("d")));
        let manifest = manifest_for("0.95.0");
        write(&journal, &manifest, None, &path).unwrap();

        // After successful write, the canonical path MUST be the only
        // artifact — no leftover temp file under the same parent dir.
        let siblings: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !siblings.iter().any(|n| n.contains(".tmp")),
            "atomic write must leave no .tmp artifact in the parent dir; \
             siblings were: {siblings:?}"
        );
        // Sanity: the canonical payload exists.
        assert!(
            path.exists(),
            "canonical journal path must exist after write"
        );
        assert!(
            load_envelope(&path).is_ok(),
            "canonical payload must be a parseable envelope (no truncation)"
        );
    }

    /// U21 / DEFECT-2026-09-23-A: when the canonical path is left in
    /// a half-written state (simulating a prior SIGKILL mid-write), a
    /// subsequent successful `write` MUST leave only one valid envelope.
    /// A non-atomic write that truncates-then-rewrites leaves a window
    /// where the canonical path is empty or partial.  This test exercises
    /// the property via concurrent reads during the write: across many
    /// writes, the reader either sees the previous full envelope or the
    /// new full envelope — never a partial payload.
    #[test]
    fn test_write_overwrites_atomic_no_partial_state_visible() {
        use std::sync::Arc;
        let tmp = TempDir::new().unwrap();
        let path = Arc::new(tmp.path().join("0.95.0.json"));
        let manifest = manifest_for("0.95.0");

        // Initial write.
        let mut j = RollbackJournal::new();
        j.record(SideEffect::CreatedDir(tmp.path().join("d1")));
        write(&j, &manifest, None, &path).unwrap();

        // Spawn a reader that spins on the path and reports what it sees.
        // Across 50 successful writes, the reader must never report a
        // partial state: either "missing" or a fully parseable envelope.
        let reader_path = Arc::clone(&path);
        let reader = std::thread::spawn(move || {
            let mut partial_seen = 0usize;
            for _ in 0..50 {
                match std::fs::read_to_string(reader_path.as_ref()) {
                    Ok(text) => {
                        // The reader must either see a valid envelope or
                        // not see the file at all — not a truncated JSON.
                        if text.is_empty() {
                            partial_seen += 1;
                        } else if serde_json::from_str::<PersistedJournal>(&text).is_err() {
                            // Any JSON-parse failure on an existing file is
                            // a partial state under our definition.
                            partial_seen += 1;
                        }
                    }
                    Err(_) => {
                        // File doesn't exist (mid-write window). Acceptable.
                    }
                }
                std::thread::yield_now();
            }
            partial_seen
        });

        // Drive 50 successful writes, each replacing the prior contents.
        for i in 0..50 {
            let mut j = RollbackJournal::new();
            j.record(SideEffect::CreatedDir(tmp.path().join(format!("d{i}"))));
            write(&j, &manifest, None, &path).unwrap();
        }

        let partial = reader.join().unwrap();
        assert_eq!(
            partial, 0,
            "atomic write must never expose a partial/truncated envelope to a \
             concurrent reader; saw {partial} partial reads in 50 iterations"
        );
        // Final state is a parseable envelope.
        assert!(load_envelope(&path).is_ok(), "final payload must parse");
    }
}
