//! Durability for the detector-reuse index (CAA-07): one atomic
//! JSON file per cache directory.
//!
//! A corrupt or foreign-schema file loads as `None`, never as an
//! error: the cache is an optimization, and losing it must cost work,
//! never correctness. Writers go through [`AtomicFileWriter`], so a
//! crash mid-save leaves the previous index intact.

use std::path::{Path, PathBuf};

use crate::application::graph_cache::{cache_dir, workspace_key};
use crate::domain::findings::detector_reuse::{DetectorReuseIndex, DetectorRunRecord};

use super::atomic_file_writer::AtomicFileWriter;

const INDEX_FILE: &str = "detector-reuse.json";

pub struct DetectorReuseStore {
    dir: PathBuf,
}

impl DetectorReuseStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store of `workspace` under `cache_root`: one directory per
    /// repository, beside its graph snapshot and revision snapshots.
    pub fn for_workspace(cache_root: &Path, workspace: &Path) -> Self {
        Self::new(
            cache_dir(cache_root)
                .join("detector-reuse")
                .join(workspace_key(workspace)),
        )
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    /// The persisted index, or `None` when absent or unreadable under
    /// a foreign shape.
    pub fn load(&self) -> Option<DetectorReuseIndex> {
        let text = std::fs::read_to_string(self.index_path()).ok()?;
        DetectorReuseIndex::from_json(&text).ok()
    }

    /// Persist the whole index atomically.
    ///
    /// # Errors
    /// I/O from creating the directory or writing the file.
    pub fn save(&self, index: &DetectorReuseIndex) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let json = serde_json::to_string(index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_atomic(&self.index_path(), json.as_bytes())
    }

    /// Record one detector run, replacing the record of the same detector
    /// over the same snapshot and keeping every other base's and
    /// detector's (within the index's per-detector retention).
    ///
    /// An index that is absent, corrupt or of a foreign schema starts over
    /// empty: the cache is an optimization, so losing it costs work and
    /// never correctness.
    ///
    /// # Errors
    /// I/O from persisting the index.
    pub fn record(&self, run: DetectorRunRecord) -> std::io::Result<()> {
        let mut index = self.load().unwrap_or_else(DetectorReuseIndex::empty);
        index.remember(run);
        self.save(&index)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut writer = AtomicFileWriter::create(path)?;
    writer.write(bytes)?;
    writer.commit()
}
