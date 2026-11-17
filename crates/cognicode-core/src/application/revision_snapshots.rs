//! Snapshots of the derived graph addressed by **revision**, not only by
//! the tree as it stands.
//!
//! ## The hole this fills
//!
//! The durable cache held one snapshot per workspace: the latest tree.
//! Switching branches evicted the other branch's work, and a second
//! checkout of a divergent commit rebuilt everything the first had
//! already derived. The snapshot is derived state, and derived state of
//! a *revision* does not stop existing when HEAD moves — it is reusable
//! the next time that revision, or a revision sharing its content, is
//! opened.
//!
//! ## What one entry is
//!
//! A [`RevisionSnapshot`] says four separable things: **who produced it**
//! ([`ProducerVersions`]: extractor, graph, facts, detector, policy — an
//! object is only reusable when every producer that made it agrees),
//! **which revision it describes** (its lineage, an opaque
//! [`RevisionHandle`]), **what content it covers** (the source digest of
//! the revision and the coverage verdict of the build), and **what it
//! derived** (a per-file extraction map, plus one resolution memo with
//! the classpath/manifiesto fingerprint that made it).
//!
//! Per-file hashes are what makes reuse honest: two branches that
//! contain the same bytes share the extraction of those bytes, while
//! semantic resolution —which depends on the classpath, not on the
//! file— is only reused when the resolution context is the same.
//!
//! ## Why there is no index
//!
//! One immutable file per lineage, named by the lineage it describes,
//! and the file repeats its lineage inside. The directory listing *is*
//! the index; an interrupted write leaves a `.tmp.` file no reader
//! consults, and a file whose inside disagrees with its name is not
//! evidence of anything. Consistency is a property of the layout, not a
//! invariant some index has to maintain.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::application::graph_cache::{cache_dir, workspace_key};
use crate::domain::scm::{ContentDigest, PathKey, RevisionHandle};
use crate::domain::value_objects::file_time::measured_nanos;

/// The shape of a revision-snapshot file. A file tagged with anything
/// else is not this shape: it is ignored by readers and swept by GC.
pub const REVISION_SCHEMA: &str = "cognicode.revision-snapshot/v1";

/// The suffix a snapshot file carries.
const SNAPSHOT_SUFFIX: &str = "snap";
/// The infix an interrupted write leaves in its temp file's name.
const TMP_INFIX: &str = ".tmp.";

/// The versions whose agreement makes a derived object reusable.
///
/// Five separable producers, because each can move without the others:
/// an extractor change invalidates extractions, a detector or policy
/// change invalidates only what that stage judged, and none of them
/// silently reuses what an older version wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerVersions {
    pub extractor: String,
    pub graph: String,
    pub facts: String,
    pub detector: String,
    pub policy: String,
}

impl ProducerVersions {
    pub fn new(
        extractor: impl Into<String>,
        graph: impl Into<String>,
        facts: impl Into<String>,
        detector: impl Into<String>,
        policy: impl Into<String>,
    ) -> Self {
        Self {
            extractor: extractor.into(),
            graph: graph.into(),
            facts: facts.into(),
            detector: detector.into(),
            policy: policy.into(),
        }
    }

    /// The one dimension that decides whether an *extraction* may be
    /// reused across snapshots: the extractor version alone. The rest
    /// of the producers judge what the extraction becomes, not the
    /// extraction itself.
    pub fn same_extractor_as(&self, other: &Self) -> bool {
        self.extractor == other.extractor
    }
}

/// What one file was extracted into, at one content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionObject {
    /// The content the object was made from. Reuse is by content, not
    /// by path: the same bytes on two branches are the same bytes.
    pub content: ContentDigest,
    /// The digest of the derived object itself (symbols, edges), as the
    /// extractor of `producer.extractor` computed it.
    pub artifact: String,
}

/// The semantic resolution of a whole revision, and the only context it
/// is valid under.
///
/// The fingerprint names the classpath/manifiesto the resolution was
/// computed against. Two snapshots whose fingerprints differ never
/// share this memo: a symbol resolved against one classpath is not
/// resolved against another, whatever the individual files say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionMemo {
    pub context_fingerprint: String,
    /// (target, resolved) pairs, in the order the resolver produced them.
    pub entries: Vec<(String, String)>,
}

/// The derived state of one revision of one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionSnapshot {
    /// Which revision this describes. The handle is opaque: this store
    /// never interprets it, it only demands that the file says the same
    /// handle it is named by.
    pub lineage: RevisionHandle,
    /// The content of the revision as a whole: what this snapshot
    /// covers, so a later claim about the same handle can be checked.
    pub source_content: ContentDigest,
    /// The coverage verdict the build declared, verbatim: full,
    /// partial, or the reason it is not the graph it is not.
    pub coverage: String,
    pub producer: ProducerVersions,
    /// Per-file extraction, keyed by path. The per-file factoring is
    /// the point: cross-branch reuse walks this map, not the graph.
    pub files: BTreeMap<PathKey, ExtractionObject>,
    /// The resolution of this revision, present only when the build
    /// resolved. Its context fingerprint is part of it: the memo can
    /// never be consulted without the context that earned it.
    pub resolution: Option<ResolutionMemo>,
}

impl RevisionSnapshot {
    /// The name of this snapshot's file, from its lineage.
    fn file_name(&self) -> String {
        lineage_file_name(self.lineage.as_str())
    }
}

/// The file name the lineage `handle` names, sanitized: the handle is
/// opaque text, and a file name is a file name.
fn lineage_file_name(handle: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(handle.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{}.{SNAPSHOT_SUFFIX}", &hex[..32])
}

/// Where every revision snapshot of one workspace lives.
#[derive(Debug, Clone)]
pub struct RevisionSnapshotStore {
    root: PathBuf,
}

impl RevisionSnapshotStore {
    /// `<cache root>/cognicode/revisions/<workspace key>`: the same
    /// workspace naming the latest-tree snapshot uses, so the same
    /// repository reached by two routes is one revision store.
    pub fn for_workspace(cache_root: &Path, workspace: &Path) -> Self {
        Self {
            root: cache_dir(cache_root)
                .join("revisions")
                .join(workspace_key(workspace)),
        }
    }

    /// The directory the entries of this workspace live in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Publishes the snapshot atomically: an interrupted write can
    /// only leave a `.tmp.` file, which no reader consults and the
    /// next GC sweeps. Re-saving the same lineage is allowed — the
    /// same revision with the same content is the same snapshot, and
    /// the write of its successor replaces it whole.
    pub fn save(&self, snapshot: &RevisionSnapshot) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let final_path = self.root.join(snapshot.file_name());
        let tmp = crate::application::graph_snapshot::tmp_path_for(&final_path);
        write_atomically(&tmp, &final_path, snapshot)
    }

    /// The snapshot of `lineage`, when this very shape and this very
    /// lineage say so.
    ///
    /// Absent is honest for: no file, a corrupt or truncated file, a
    /// file naming a lineage it does not describe, and — the
    /// explicit-invalidation half of the contract — a file written by
    /// another producer than `current_producer` claims. Nothing is
    /// deleted here; it is simply not evidence for this analyzer.
    pub fn load(
        &self,
        lineage: &RevisionHandle,
        current_producer: &ProducerVersions,
    ) -> Option<RevisionSnapshot> {
        let snapshot = read_snapshot(&self.root.join(lineage_file_name(lineage.as_str()))).ok()?;
        // The file is named by the lineage it is asked for, and it has
        // to agree: a file whose inside describes another revision is
        // not evidence for this one, whatever else it holds.
        if &snapshot.lineage != lineage {
            return None;
        }
        // Explicit invalidation: this producer does not serve what
        // another producer wrote, and does not say so by deleting it.
        if &snapshot.producer != current_producer {
            return None;
        }
        Some(snapshot)
    }

    /// The lineage handles this store holds snapshots for, as written
    /// inside the files (the names only address them).
    pub fn lineages(&self) -> io::Result<Vec<String>> {
        let mut found = Vec::new();
        for entry in entries_of(&self.root)? {
            if let Some(snapshot) = read_snapshot(&entry)
                .ok()
                .filter(|s| s.file_name() == name_of(&entry))
            {
                found.push(snapshot.lineage.as_str().to_string());
            }
        }
        Ok(found)
    }

    /// Sweeps the store down to `quota` unreferenced entries: the
    /// referenced lineages are set aside first and are never touched,
    /// and of the rest the newest `quota` survive while every older
    /// snapshot — and every temp file, which is not an entry at all —
    /// is removed. `0` means "keep only what is referenced".
    ///
    /// A snapshot that cannot be read cannot be named by a lineage, so
    /// it is unreferenced by construction and gets swept; the report
    /// names the lineages it removed.
    pub fn gc(&self, referenced: &BTreeSet<String>, quota: usize) -> io::Result<GcReport> {
        let mut report = GcReport::default();
        // A temp file is not an entry: it was never published, and it
        // is sweepable the moment a sweep runs.
        if let Ok(read) = std::fs::read_dir(&self.root) {
            for entry in read.flatten() {
                let path = entry.path();
                if name_of(&path).contains(TMP_INFIX) {
                    std::fs::remove_file(&path)?;
                }
            }
        }
        let mut leftovers: Vec<(Option<i64>, PathBuf, Option<RevisionSnapshot>)> = Vec::new();
        for entry in entries_of(&self.root)? {
            let name = name_of(&entry);
            let snapshot = read_snapshot(&entry).ok().filter(|s| s.file_name() == name);
            let referenced_lineage = snapshot
                .as_ref()
                .is_some_and(|s| referenced.contains(s.lineage.as_str()));
            if referenced_lineage {
                continue;
            }
            // A file whose date cannot be read has no date: `None` sorts
            // before every measured one, so it is swept first.
            let stamp = measured_nanos(std::fs::metadata(&entry).and_then(|m| m.modified()).ok());
            leftovers.push((stamp, entry, snapshot));
        }
        // Oldest first: the quota is spent on the newest survivors.
        leftovers.sort_by_key(|(stamp, _, _)| *stamp);
        let sweep_all = leftovers.len().saturating_sub(quota);
        for (_, path, snapshot) in leftovers.into_iter().take(sweep_all) {
            if let Some(snapshot) = snapshot {
                report.removed.push(snapshot.lineage.as_str().to_string());
            }
            std::fs::remove_file(&path)?;
        }
        report.removed.sort();
        Ok(report)
    }
}

/// What one sweep did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GcReport {
    pub removed: Vec<String>,
}

/// The verdict about the workspace's legacy single-snapshot cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyCache {
    /// Nothing to migrate: no file.
    Absent,
    /// The file is what the current analyzer would write: leave it.
    Current,
    /// The file says another producer made it (or its shape cannot be
    /// read at all): it is not evidence, and [`invalidate_legacy`] may
    /// remove it.
    Stale(PathBuf),
}

/// Looks at the workspace's latest-tree snapshot and says whether its
/// producer is the one standing here.
pub fn inspect_legacy(cache_root: &Path, workspace: &Path) -> LegacyCache {
    use crate::application::graph_cache::snapshot_path;
    use crate::application::graph_snapshot::{canonical_workspace, load_durable_snapshot};
    use crate::domain::value_objects::snapshot_key::SnapshotKey;

    let path = snapshot_path(cache_root, workspace);
    if !path.exists() {
        return LegacyCache::Absent;
    }
    let current = SnapshotKey::for_current_analyzer(canonical_workspace(workspace));
    match load_durable_snapshot(&path) {
        Some((key, _, _)) if key.same_producer_as(&current) => LegacyCache::Current,
        _ => LegacyCache::Stale(path),
    }
}

/// Removes a stale legacy snapshot, reporting whether one was there.
/// A current cache is never removed by this door: invalidation is a
/// verdict about a producer, not a habit.
pub fn invalidate_legacy(cache_root: &Path, workspace: &Path) -> io::Result<bool> {
    match inspect_legacy(cache_root, workspace) {
        LegacyCache::Stale(path) => {
            std::fs::remove_file(&path)?;
            Ok(true)
        }
        LegacyCache::Absent | LegacyCache::Current => Ok(false),
    }
}

/// Whether the extraction of `path` may be reused from `base` by `head`:
/// the same bytes, judged by the same extractor. Nothing else —not the
/// graph version, not the branch— can forbid it, and nothing else
/// grants it.
pub fn reuse_extraction<'a>(
    base: &'a RevisionSnapshot,
    head: &RevisionSnapshot,
    path: &PathKey,
) -> Option<&'a ExtractionObject> {
    if !base.producer.same_extractor_as(&head.producer) {
        return None;
    }
    let in_base = base.files.get(path)?;
    let in_head = head.files.get(path)?;
    // The claim to reuse is a claim about content: the two snapshots
    // have to agree that this path holds these very bytes.
    if in_base.content == in_head.content {
        Some(in_base)
    } else {
        None
    }
}

/// Whether `head` may use `base`'s resolution memo: both have one, and
/// both earned it against the same classpath/manifiesto fingerprint.
pub fn resolution_reusable(base: &RevisionSnapshot, head: &RevisionSnapshot) -> bool {
    match (&base.resolution, &head.resolution) {
        (Some(b), Some(h)) => b.context_fingerprint == h.context_fingerprint,
        _ => false,
    }
}

// ----- the layout: names, reading, writing --------------------------

/// The file name of a directory entry.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The snapshot files of a directory: plain entries whose name ends in
/// the suffix, never a temp file. A missing directory holds nothing.
fn entries_of(root: &Path) -> io::Result<Vec<PathBuf>> {
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut entries = Vec::new();
    for entry in read {
        let path = entry?.path();
        let name = name_of(&path);
        if name.ends_with(&format!(".{SNAPSHOT_SUFFIX}")) && !name.contains(TMP_INFIX) {
            entries.push(path);
        }
    }
    Ok(entries)
}

/// The envelope as it goes on disk: the schema, then the four claims
/// as plain strings, so the file can be read without trusting any
/// type in it — the constructors re-check every one.
type Envelope = (
    String,                                   // schema
    String,                                   // lineage
    String,                                   // source content digest
    String,                                   // coverage
    (String, String, String, String, String), // producer versions
    Vec<(String, String, String)>,            // (path, content, artifact)
    Option<(String, Vec<(String, String)>)>,  // (context, entries)
);

fn encode_envelope(snapshot: &RevisionSnapshot) -> io::Result<Vec<u8>> {
    use bincode::config::standard;
    use bincode::serde::encode_to_vec;

    let envelope = (
        REVISION_SCHEMA.to_string(),
        snapshot.lineage.as_str().to_string(),
        snapshot.source_content.as_hex().to_string(),
        snapshot.coverage.clone(),
        (
            snapshot.producer.extractor.clone(),
            snapshot.producer.graph.clone(),
            snapshot.producer.facts.clone(),
            snapshot.producer.detector.clone(),
            snapshot.producer.policy.clone(),
        ),
        snapshot
            .files
            .iter()
            .map(|(path, object)| {
                (
                    path.as_str().to_string(),
                    object.content.as_hex().to_string(),
                    object.artifact.clone(),
                )
            })
            .collect::<Vec<(String, String, String)>>(),
        snapshot
            .resolution
            .as_ref()
            .map(|memo| (memo.context_fingerprint.clone(), memo.entries.clone())),
    );
    encode_to_vec(envelope, standard())
        .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Reads one entry. Any failure to agree with this shape — wrong
/// schema, unreadable bytes, a claim the domain refuses — is the
/// honest `io::ErrorKind::InvalidData`: not evidence, rebuild.
fn read_snapshot(path: &Path) -> io::Result<RevisionSnapshot> {
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > crate::application::graph_snapshot::MAX_SNAPSHOT_BYTES {
        return Err(std::io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot too large",
        ));
    }
    let mut bytes = Vec::new();
    file.take(crate::application::graph_snapshot::MAX_SNAPSHOT_BYTES)
        .read_to_end(&mut bytes)?;
    let (envelope, consumed): (Envelope, usize) = decode_from_slice(&bytes, standard())
        .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e))?;
    if consumed != bytes.len() || envelope.0 != REVISION_SCHEMA {
        return Err(std::io::Error::new(
            io::ErrorKind::InvalidData,
            "wrong shape",
        ));
    }
    let lineage = RevisionHandle::new(&envelope.1)
        .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let source_content = ContentDigest::from_hex(&envelope.2)
        .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut files = BTreeMap::new();
    for (p, content, artifact) in envelope.5 {
        let path_key = PathKey::new(&p)
            .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let digest = ContentDigest::from_hex(&content)
            .map_err(|e| std::io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        files.insert(
            path_key,
            ExtractionObject {
                content: digest,
                artifact,
            },
        );
    }
    Ok(RevisionSnapshot {
        lineage,
        source_content,
        coverage: envelope.3,
        producer: ProducerVersions::new(
            envelope.4.0,
            envelope.4.1,
            envelope.4.2,
            envelope.4.3,
            envelope.4.4,
        ),
        files,
        resolution: envelope.6.map(|(context, entries)| ResolutionMemo {
            context_fingerprint: context,
            entries,
        }),
    })
}

/// Writes the bytes to `tmp` and renames onto the final path: the only
/// way an entry appears in this store is by being whole.
fn write_atomically(tmp: &Path, final_path: &Path, snapshot: &RevisionSnapshot) -> io::Result<()> {
    let bytes = encode_envelope(snapshot)?;
    std::fs::write(tmp, &bytes)?;
    std::fs::rename(tmp, final_path)
}
