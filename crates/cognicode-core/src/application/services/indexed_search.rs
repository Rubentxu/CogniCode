//! The edge between a content search and the trigram index.
//!
//! The search walks the tree and reads files; this module decides, file by
//! file, whether the index lets it skip the read, and keeps the index up to
//! date with what the search reads anyway. The index never answers a search:
//! it only gates the reads, so every match comes from a file read during the
//! search.

use std::fs::Metadata;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::application::dto::TextIndexReport;
use crate::application::services::text_index::{
    FileStamp, GateVerdict, MAX_INDEXED_BYTES, TOKENIZER_VERSION, TextIndex, TextIndexConfig,
    TrigramPlan, plan_literal, plan_regex, trigrams_of,
};
use crate::domain::value_objects::file_time::epoch_nanos;

/// A text index shared by the handles of a service, with the policy that says
/// what stays out of it.
#[derive(Clone)]
pub(crate) struct SharedTextIndex {
    index: Arc<RwLock<TextIndex>>,
    config: Arc<TextIndexConfig>,
}

/// What the search does with a file.
enum FileDecision {
    /// The index vouches that the file holds no match: do not read it.
    Skip,
    /// Read the file and verify. When `indexing` is set, the index could not
    /// vouch for the file: what was read is recorded.
    Read { indexing: Option<Observation> },
}

/// What reading a file for the search gave.
pub(crate) enum FileRead {
    /// The index vouched that the file holds no match, so it was not read.
    Skipped,
    /// The file opened and is not text: it has nothing to search, and it is
    /// not a hole.
    NotText,
    /// The text of the file, to be verified against the pattern.
    Text(String),
    /// The file could not be read: a hole in the search.
    Failed(std::io::Error),
}

/// A file as it was seen before it was read, to be recorded with its content.
struct Observation {
    stamp: FileStamp,
    observed_ns: i128,
}

/// The index, as one search uses it. A walk over no index reads every file
/// and reports nothing.
pub(crate) struct IndexedWalk<'a> {
    shared: Option<&'a SharedTextIndex>,
    alternatives: Option<Vec<Vec<u32>>>,
    walk: u64,
    report: TextIndexReport,
}

impl SharedTextIndex {
    pub(crate) fn new(config: TextIndexConfig) -> Self {
        Self {
            index: Arc::new(RwLock::new(TextIndex::with_quiet_period(
                config.quiet_period,
            ))),
            config: Arc::new(config),
        }
    }

    /// The number of files the index holds a document for.
    #[cfg(test)]
    pub(crate) fn documents(&self) -> usize {
        self.index.read().len()
    }
}

impl<'a> IndexedWalk<'a> {
    /// Starts using `shared`, if there is an index, for one search of
    /// `pattern`.
    pub(crate) fn over(shared: Option<&'a SharedTextIndex>, pattern: &str, is_regex: bool) -> Self {
        let mut report = TextIndexReport {
            tokenizer_version: TOKENIZER_VERSION,
            ..TextIndexReport::default()
        };
        let mut alternatives = None;
        let mut walk = 0;
        if let Some(shared) = shared {
            match if is_regex {
                plan_regex(pattern)
            } else {
                plan_literal(pattern)
            } {
                TrigramPlan::Narrow(plan) => {
                    report.used = true;
                    alternatives = Some(plan);
                }
                TrigramPlan::Unusable(reason) => report.reason = Some(reason.to_string()),
            }
            walk = shared.index.read().begin_walk();
        }
        Self {
            shared,
            alternatives,
            walk,
            report,
        }
    }

    /// Reads the file at `path`, whose metadata is `metadata`, unless the
    /// index vouches that it holds no match; keeps the index up to date with
    /// what was read.
    pub(crate) fn read_text(&mut self, path: &Path, metadata: Option<&Metadata>) -> FileRead {
        let indexing = match self.decide(path, metadata) {
            FileDecision::Skip => return FileRead::Skipped,
            FileDecision::Read { indexing } => indexing,
        };
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.record(path, indexing, Some(&text));
                FileRead::Text(text)
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                self.record(path, indexing, None);
                FileRead::NotText
            }
            Err(error) => {
                if let (Some(shared), Some(_)) = (self.shared, &self.alternatives) {
                    shared.index.write().remove(path);
                }
                FileRead::Failed(error)
            }
        }
    }

    fn decide(&mut self, path: &Path, metadata: Option<&Metadata>) -> FileDecision {
        let (Some(shared), Some(alternatives)) = (self.shared, &self.alternatives) else {
            return FileDecision::Read { indexing: None };
        };
        let observed_ns = now_ns();
        let stamp = metadata.map(stamp_of);
        let kept_out = shared.config.excludes(path)
            || stamp.is_none_or(|stamp| stamp.len > MAX_INDEXED_BYTES as u64);
        let Some(stamp) = stamp.filter(|_| !kept_out) else {
            self.report.read_directly += 1;
            shared.index.write().remove(path);
            return FileDecision::Read { indexing: None };
        };
        // Too recent to vouch for, now or by the next search: indexing it
        // would only cost a tokenization that nothing can use yet.
        if stamp.is_racy(observed_ns, shared.config.quiet_period.as_nanos() as i128) {
            self.report.read_directly += 1;
            return FileDecision::Read { indexing: None };
        }
        match shared
            .index
            .read()
            .judge(path, &stamp, alternatives, self.walk)
        {
            GateVerdict::Candidate => {
                self.report.candidates += 1;
                FileDecision::Read { indexing: None }
            }
            GateVerdict::NotCandidate => {
                self.report.skipped += 1;
                FileDecision::Skip
            }
            GateVerdict::Unknown => FileDecision::Read {
                indexing: Some(Observation { stamp, observed_ns }),
            },
        }
    }

    /// Indexes what the search read of the file. `content` is `None` for a
    /// file that opened and is not text: it holds nothing to find, and the
    /// index remembers that so it is not opened again.
    fn record(&mut self, path: &Path, seen: Option<Observation>, content: Option<&str>) {
        let (Some(shared), Some(seen)) = (self.shared, seen) else {
            return;
        };
        let trigrams = content.map(trigrams_of).unwrap_or_default();
        let mut index = shared.index.write();
        if content.is_some_and(|text| text.len() > MAX_INDEXED_BYTES) {
            // It grew between the measurement and the read.
            index.remove(path);
            self.report.read_directly += 1;
            return;
        }
        index.upsert(
            path.to_path_buf(),
            seen.stamp,
            seen.observed_ns,
            &trigrams,
            self.walk,
        );
        self.report.indexed_now += 1;
    }

    /// Ends the search and reports what the index did; `None` without an
    /// index.
    ///
    /// When the walk covered everything under `root` (`complete`: nothing
    /// truncated, unread or filtered out), the documents it did not go
    /// through are for files that are gone, and they are dropped.
    pub(crate) fn finish(self, root: &Path, complete: bool) -> Option<TextIndexReport> {
        let shared = self.shared?;
        if complete && self.alternatives.is_some() {
            shared.index.write().sweep(root, self.walk);
        }
        Some(self.report)
    }
}

fn now_ns() -> i128 {
    i128::from(epoch_nanos(SystemTime::now()))
}

fn stamp_of(metadata: &Metadata) -> FileStamp {
    let modified_ns = metadata
        .modified()
        .map(|time| i128::from(epoch_nanos(time)))
        .unwrap_or(i128::MAX);
    FileStamp {
        len: metadata.len(),
        modified_ns,
        changed_ns: status_change_ns(metadata).unwrap_or(modified_ns),
    }
}

#[cfg(unix)]
fn status_change_ns(metadata: &Metadata) -> Option<i128> {
    use std::os::unix::fs::MetadataExt;
    Some(i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()))
}

#[cfg(not(unix))]
fn status_change_ns(_metadata: &Metadata) -> Option<i128> {
    None
}
