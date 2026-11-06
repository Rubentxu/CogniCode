//! The durable snapshot of the graph, and the one decision that says whether
//! it is served or calculated again.
//!
//! ## What this owns
//!
//! The snapshot is the product's own state: it lives in the user's cache,
//! keyed by the workspace, and it names the analyzer that calculated it. It
//! used to live inside the analysed tree, and it was owned by the MCP's
//! interface layer — so the CLI could not reach it, and the CLI rebuilt the
//! whole graph on every run.
//!
//! Here the snapshot and the decision live together, in the application
//! layer: both interfaces call the same function, and what one of them
//! calculates the other one serves.
//!
//! ## The decision
//!
//! `load_or_build_graph` serves the snapshot when it can —the tree has not
//! changed since it was calculated, and the analyzer that calculated it is
//! the one standing here— and calculates the graph when it cannot. A
//! snapshot that cannot be served is not evidence of anything: it is
//! rebuilt, and the reason is in the log.

use crate::application::analysis_basis::GraphSource;
use crate::application::error::AppResult;
use crate::application::graph_cache;
use crate::application::services::analysis_service::{AnalysisService, TreeFile, TreeState};
use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::domain::value_objects::{AnalyzerBuild, SNAPSHOT_SCHEMA, SnapshotKey};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The graph the product answers with, and where it came from.
pub struct LoadedGraph {
    /// The graph, as the service holds it now.
    pub graph: Arc<CallGraph>,
    /// The manifest that says which files the graph was built from.
    ///
    /// `None` when the graph was served without one: a session that kept
    /// its own graph does not have to describe it again.
    pub manifest: Option<FileManifest>,
    /// Where the graph came from.
    ///
    /// The answer has to say which of the three it was: a graph calculated
    /// now and a graph calculated before are the same answer only when
    /// the tree did not change, and the consumer is the one who has to
    /// know that the product checked —and who checked.
    pub source: GraphSource,
    /// The tree as it was when the answer was decided.
    ///
    /// The holes the tree has RIGHT NOW travel here. The persisted graph
    /// cannot know about them: a hole is not one of the files the graph
    /// is built from, so it is not in the manifest, and the freshness
    /// check cannot see it. The verdict of a served answer is therefore
    /// computed against the tree the consumer is asking about, not
    /// against the tree the snapshot was taken from.
    pub tree: TreeState,
    /// What the answer covers.
    ///
    /// It is measured here, once, by the one owner of that decision: the
    /// source says where the graph came from, the report is the report of
    /// the build of THIS call, the holes are the ones the tree has NOW,
    /// and the material says whether the tree holds anything to read.
    ///
    /// Every surface that answers with this graph publishes this
    /// coverage, and they all ask the same function — two rules that look
    /// alike are two rules, and the second one goes stale while the first
    /// one is kept.
    pub coverage: crate::application::services::analyze::Coverage,
}

/// The answer, with the coverage it was measured with.
///
/// The coverage is decided by [`coverage_verdict`] —the one owner of that
/// decision— and the service remembers it, because the next question of
/// the same session is answered with the graph that is already held, and
/// that graph was measured when it was loaded.
///
/// [`coverage_verdict`]: crate::application::services::analyze::coverage_verdict
fn loaded(
    service: &AnalysisService,
    graph: Arc<CallGraph>,
    manifest: Option<FileManifest>,
    source: GraphSource,
    tree: TreeState,
) -> LoadedGraph {
    let coverage = crate::application::services::analyze::coverage_verdict(
        source,
        service.get_last_build_report().as_ref(),
        &tree.holes,
        crate::application::services::analyze::holds_material(graph.symbol_count()),
    );
    service.set_last_coverage(coverage.clone());
    LoadedGraph {
        graph,
        manifest,
        source,
        tree,
        coverage,
    }
}

/// The graph of `project_dir`, served from the snapshot when it can be and
/// calculated when it cannot.
///
/// One decision, for every interface: the MCP and the CLI call this, so
/// what one of them calculates the other one serves.
///
/// `session_manifest` is the manifest of the graph the service already
/// holds, when it holds one: the session's own record of what it read. It
/// is how a session that already calculated the graph knows that the tree
/// has not changed since. `None` means "this session has calculated
/// nothing yet".
///
/// `force_rebuild` says the caller wants the tree read again whatever the
/// snapshot says. The CLI's `--rebuild` is that flag.
pub fn load_or_build_graph(
    service: &AnalysisService,
    project_dir: &Path,
    cache_root: Option<&Path>,
    session_manifest: Option<&FileManifest>,
    force_rebuild: bool,
) -> AppResult<LoadedGraph> {
    let snapshot_path = cache_root.map(|root| graph_cache::snapshot_path(root, project_dir));

    if !force_rebuild {
        // The session holds a graph already: it is the answer, as long as
        // the tree has not changed since it was calculated.
        let session_holds_a_graph = {
            let graph = service.graph_cache().get();
            graph.symbol_count() > 0 || graph.edge_count() > 0
        };
        if session_holds_a_graph {
            if let Some(manifest) = session_manifest {
                let tree = service.tree_state(project_dir);
                let vouched = vouched_for(service, manifest, project_dir, &tree);
                if vouched || !is_manifest_stale(manifest, project_dir, &tree.files) {
                    return Ok(loaded(
                        service,
                        service.get_project_graph(),
                        None,
                        if vouched {
                            GraphSource::Daemon
                        } else {
                            GraphSource::Cache
                        },
                        tree,
                    ));
                }
            }
            // The session's graph is stale, or nothing says what it read:
            // the tree is read again.
        } else if let Some(path) = snapshot_path.as_ref().filter(|path| path.exists())
            && let Some((key, graph, manifest)) = load_durable_snapshot(path)
        {
            // The session holds nothing: the durable snapshot may still
            // hold what another session calculated over the same tree.
            //
            // The first question is about the producer, and it costs
            // nothing: it is answered without reading the tree. A
            // snapshot written by another analyzer —another version,
            // another commit, another capability set— describes a graph
            // this analyzer did not calculate, so it is not evidence of
            // anything here.
            let current_key = SnapshotKey::new(
                canonical_workspace(project_dir),
                AnalyzerBuild::current(),
                SNAPSHOT_SCHEMA,
            );
            if key.same_producer_as(&current_key) {
                // Scan source files and compare hashes. The walk that
                // decides the freshness also says what holes the tree
                // has now, so it is done once.
                let tree = service.tree_state(project_dir);
                let vouched = vouched_for(service, &manifest, project_dir, &tree);
                if vouched || !is_manifest_stale(&manifest, project_dir, &tree.files) {
                    service.graph_cache().replace(graph.clone());
                    return Ok(loaded(
                        service,
                        Arc::new(graph),
                        Some(manifest),
                        if vouched {
                            GraphSource::Daemon
                        } else {
                            GraphSource::Cache
                        },
                        tree,
                    ));
                }
            } else {
                tracing::warn!(
                    "the durable snapshot was produced by another analyzer \
                     (version {}, commit {:?}); it is rebuilt",
                    key.analyzer.version,
                    key.analyzer.commit
                );
            }
        }
    }

    // The manifest is taken BEFORE the build. A manifest written after
    // the graph could describe a tree the graph never saw: a file that
    // appeared between the two would be in the manifest and missing from
    // the graph, and the next call would find the manifest fresh and
    // serve a graph that does not cover the tree. Taken first, the
    // manifest is always at or before the graph, so anything that changes
    // in between invalidates it.
    let tree = service.tree_state(project_dir);
    let manifest = build_manifest(project_dir, &tree.files);
    service.build_project_graph(project_dir)?;

    // Only persist a snapshot when the build is Complete. A Partial build
    // (unreadable/failed files) must not leave behind a snapshot that a
    // later session could mistake for full coverage — rebuild-every-time
    // is the safe default.
    let build_complete = service.last_build_was_complete();
    // The manifest the caller keeps is the one of a build that read the
    // whole tree: a partial build has no manifest that describes the
    // whole tree, and keeping it would let the next session serve a
    // partial graph as if it were fresh.
    let manifest_to_keep = if build_complete {
        match snapshot_path.as_ref() {
            Some(path) => {
                // The snapshot is written with the key of the analyzer
                // that just calculated it. A snapshot that does not name
                // its producer cannot be told apart from one written by
                // another build.
                let key = SnapshotKey::new(
                    canonical_workspace(project_dir),
                    AnalyzerBuild::current(),
                    SNAPSHOT_SCHEMA,
                );
                let graph = service.get_project_graph();
                if let Err(e) = save_durable_snapshot(path, &key, &graph, &manifest) {
                    tracing::warn!("durable snapshot save failed: {}", e);
                }
                Some(manifest)
            }
            None => {
                tracing::warn!(
                    "no cache root is configured: the graph snapshot is not persisted, \
                     and every request rebuilds the graph"
                );
                Some(manifest)
            }
        }
    } else {
        if let Some(path) = snapshot_path.as_ref() {
            // A stale/partial snapshot must not survive a failed rebuild.
            let _ = std::fs::remove_file(path);
        }
        None
    };

    Ok(loaded(
        service,
        service.get_project_graph(),
        manifest_to_keep,
        GraphSource::Built,
        tree,
    ))
}

/// Whether somebody who is watching the tree says that `manifest` still
/// describes it.
///
/// Two things have to hold for the answer to be worth anything:
///
/// 1. The tree the watcher is watching is the tree the manifest was taken
///    from. The modification times decide that, and they are free: the
///    walk already read them. A file that changed after the manifest was
///    taken has a later time, and a manifest that does not know a file
///    is a manifest of another tree.
/// 2. The watcher is talking about THIS manifest. The digest of the
///    manifest is what binds the two: a watcher that is watching another
///    build says nothing about this one.
///
/// When both hold, the content of the tree does not have to be read to
/// know that the index still describes it. That is the whole point of the
/// watcher: it is the one that has to notice the change that leaves the
/// modification time alone.
fn vouched_for(
    service: &AnalysisService,
    manifest: &FileManifest,
    project_dir: &Path,
    tree: &TreeState,
) -> bool {
    let Some(oracle) = service.freshness_oracle() else {
        return false;
    };
    if !manifest_mtimes_agree(manifest, project_dir, &tree.files) {
        return false;
    }
    oracle.vouches_for(manifest)
}

/// The digest of a manifest: the tree state it describes.
///
/// The manifest is a map of files to what was read from them, and this
/// digest is over that map —sorted by path, because the map itself is
/// not ordered, and two manifests of the same tree state have to have
/// the same digest whatever order they were built in.
///
/// It is the thing that binds a claim to a tree state: a watcher that
/// says "my index is fresh" says it about a digest, and a client can
/// tell whether that digest is the one of the manifest it holds.
pub fn manifest_digest(manifest: &FileManifest) -> String {
    use sha2::{Digest, Sha256};

    let mut entries: Vec<(
        &PathBuf,
        &crate::domain::value_objects::file_manifest::FileEntry,
    )> = manifest.entries.iter().collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));

    let mut hasher = Sha256::new();
    for (relative, entry) in entries {
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.content_hash.as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.mtime.unwrap_or_default().to_string().as_bytes());
        hasher.update(b"\n");
    }
    hex_digest(&hasher.finalize())
}

/// The digest of the sources a manifest describes.
///
/// The manifest says what was read from each file; this digest is over
/// that —the relative paths and the content hashes, sorted, because the
/// map itself is not ordered— and it is the identity of the *sources*:
/// two analyses that read the same files with the same contents have the
/// same digest, and a file whose content changed has a different one.
///
/// It is not the digest of the tree *state*: the times a file was read at
/// are a different fact, and they are checked against the tree by
/// [`manifest_mtimes_agree`]. A digest that mixed the two would say that
/// a file nobody touched was read again.
pub fn source_digest(manifest: &FileManifest) -> String {
    use sha2::{Digest, Sha256};

    let mut entries: Vec<(
        &PathBuf,
        &crate::domain::value_objects::file_manifest::FileEntry,
    )> = manifest.entries.iter().collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));

    let mut hasher = Sha256::new();
    for (relative, entry) in entries {
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.content_hash.as_bytes());
        hasher.update(b"\n");
    }
    hex_digest(&hasher.finalize())
}

/// Atomically persist the graph + manifest snapshot to
/// `db_path` (temp file + rename, so an interrupted write never leaves a
/// half-written snapshot that could be mistaken for valid evidence).
///
/// The temp file uses a per-process unique name
/// (`cache.tmp.<pid>.<seq>`), not a fixed slot. Two writers on the same
/// workspace never share a temp path; an interrupted writer leaves a
/// file whose name identifies the dying process for post-mortem.
///
/// The snapshot carries the **key** —who produced it, over what, and with
/// which shape— next to the graph and the manifest. Without it, a snapshot
/// is a graph with no author, and any other build would serve it as its own.
pub fn save_durable_snapshot(
    db_path: &Path,
    key: &SnapshotKey,
    graph: &crate::domain::aggregates::call_graph::CallGraph,
    manifest: &crate::domain::value_objects::file_manifest::FileManifest,
) -> std::io::Result<()> {
    use bincode::config::standard;
    use bincode::serde::encode_to_vec;

    // The version says what the snapshot means, and the meaning of a
    // snapshot changed three times: a graph is now only "complete" when its
    // references are tied too, and a snapshot written before that says
    // complete about a graph that had holes in it; the manifest now records
    // a modification time that can be absent, a shape the previous layout
    // cannot hold; and the snapshot now names the analyzer that produced
    // it, a shape the previous layout cannot hold either. A snapshot of an
    // older version is therefore not evidence of anything: it is rebuilt.
    let snapshot = (SNAPSHOT_SCHEMA, key, graph, manifest);
    let bytes = encode_to_vec(snapshot, standard())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(db_path);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, db_path)
}

/// Returns a unique tmp path for `db_path`.
///
/// Each call appends `<pid>.<seq>` so concurrent writers on the same
/// workspace never collide on the temp slot. The sequence counter is
/// an atomic monotonic per-process counter (no two consecutive calls
/// from the same process produce the same path even after many
/// snapshots).
pub fn tmp_path_for(db_path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = format!(
        "{}.tmp.{}.{}",
        db_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("graph.cache"),
        pid,
        seq
    );
    db_path.with_file_name(file_name)
}

/// Load a durable snapshot. Returns `None` on any
/// parse failure — a corrupt snapshot is treated as absent (rebuild),
/// never as valid evidence.
///
/// The key comes back with the graph: who produced it is part of what the
/// snapshot says, and the caller decides whether that producer is the one
/// standing here.
pub fn load_durable_snapshot(
    db_path: &Path,
) -> Option<(
    SnapshotKey,
    crate::domain::aggregates::call_graph::CallGraph,
    crate::domain::value_objects::file_manifest::FileManifest,
)> {
    load_durable_snapshot_capped(db_path, MAX_SNAPSHOT_BYTES)
}

/// Largest snapshot the reader will pull into memory. The bytes come from
/// a file on disk that this run did not necessarily write: a truncated
/// write, a foreign file or a corrupted cache are all possible, and
/// bincode cannot allocate beyond what the bytes it is given can back, so
/// bounding the bytes bounds the decode.
pub const MAX_SNAPSHOT_BYTES: u64 = 256 * 1024 * 1024;

pub fn load_durable_snapshot_capped(
    db_path: &Path,
    max_bytes: u64,
) -> Option<(
    SnapshotKey,
    crate::domain::aggregates::call_graph::CallGraph,
    crate::domain::value_objects::file_manifest::FileManifest,
)> {
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;

    use std::io::Read;

    let file = std::fs::File::open(db_path).ok()?;
    if file.metadata().ok()?.len() > max_bytes {
        return None;
    }
    // The length was read before the contents: a file that grows in between
    // is still cut at the cap, and a cut snapshot fails to decode.
    let mut bytes = Vec::new();
    file.take(max_bytes).read_to_end(&mut bytes).ok()?;
    let (snapshot, _): ((String, _, _, _), usize) = decode_from_slice(&bytes, standard()).ok()?;
    if snapshot.0 != SNAPSHOT_SCHEMA {
        return None;
    }
    Some((snapshot.1, snapshot.2, snapshot.3))
}

/// El workspace canónico: el mismo string en la clave y en la base.
///
/// Un solo sitio lo decide, para que la clave que se guarda y la que se
/// compara no puedan separarse por una canonicalización distinta.
pub fn canonical_workspace(project_dir: &Path) -> String {
    project_dir
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| project_dir.to_string_lossy().to_string())
}

/// Whether every file the manifest knows is still there, with the same
/// modification time, and no file is missing from the manifest.
///
/// This is the cheap half of the freshness check: the walk already read
/// the times, so comparing them costs nothing. It cannot tell a file
/// whose content changed with the time left alone —that is what the
/// content hash is for— but it does catch every ordinary change, and it
/// is what decides whether the expensive half is worth paying.
pub fn manifest_mtimes_agree(
    manifest: &crate::domain::value_objects::file_manifest::FileManifest,
    project_dir: &Path,
    files: &[TreeFile],
) -> bool {
    // The files that can change the graph are decided by the same rule
    // the graph builder uses. A manifest that covered a different set
    // would miss the files it does not know about, and a change in one
    // of those would leave the cached graph in place while the product
    // reported a complete build over sources that no longer exist.
    let mut checked = 0usize;
    for file in files {
        let path = &file.path;
        let rel = path.strip_prefix(project_dir).unwrap_or(path);
        checked += 1;
        match manifest.entries.get(rel) {
            Some(entry) if entry.mtime == file.mtime_millis => {}
            _ => return false, // new, modified, or missing from manifest
        }
    }
    // If manifest has more entries than files checked, files were deleted
    checked == manifest.entries.len()
}

/// Checks whether any source file has changed since the manifest was saved.
///
/// Staleness is decided on three independent signals, in order of cost:
///
/// 1. Filesystem walk to enumerate the current source files
/// 2. Per-file mtime comparison (cheap)
/// 3. Content hash comparison (expensive, but authoritative)
///
/// A file that is modified, added, or deleted invalidates the manifest.
///
/// A source file that cannot be read at all (permissions revoked,
/// deleted mid-scan) is treated as a change: serving the cached graph
/// would hide the loss of coverage behind a stale "complete" result.
///
/// New files not in the manifest and files in the manifest but absent
/// from disk also invalidate the snapshot.
///
/// The files are passed in rather than walked here: the walk is the
/// cost the cache exists to avoid paying twice, and the same walk also
/// says what holes the tree has right now.
pub fn is_manifest_stale(
    manifest: &crate::domain::value_objects::file_manifest::FileManifest,
    project_dir: &Path,
    files: &[TreeFile],
) -> bool {
    if !manifest_mtimes_agree(manifest, project_dir, files) {
        return true;
    }

    for file in files {
        let path = &file.path;
        let rel = path.strip_prefix(project_dir).unwrap_or(path);
        // Mtime alone is not sufficient. A tool
        // can rewrite the file bytes while keeping the original
        // timestamp (e.g. restoring a backup, partial restore,
        // `touch -r`). Compare the on-disk content_hash against
        // the stored one to detect that case.
        //
        // A file whose content can no longer be
        // read (e.g. permissions revoked after the snapshot)
        // invalidates the snapshot — serving it would hide the
        // loss of coverage behind a stale "complete" result.
        //
        // A file whose clock nobody read has no time to compare:
        // both sides are absent, the times agree, and the file
        // does not invalidate the snapshot by itself. What decides
        // is the content hash.
        if !content_matches_manifest(manifest, rel, path) {
            return true;
        }
    }
    false
}

/// Whether the file at `path` still has the content the manifest recorded.
///
/// The content hash is what decides; the modification time is only a
/// hint. A file the manifest does not know, and a file that cannot be
/// read, are not known to be unchanged.
///
/// This is the one place that compares a file against what the
/// manifest says about it, so the freshness of the snapshot and the
/// decision of an incremental reindex cannot disagree about the same
/// file.
pub fn content_matches_manifest(
    manifest: &crate::domain::value_objects::file_manifest::FileManifest,
    rel_path: &Path,
    path: &Path,
) -> bool {
    use sha2::{Digest, Sha256};

    let Some(entry) = manifest.entries.get(rel_path) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    hex_digest(&Sha256::digest(&bytes)) == entry.content_hash
}

/// Builds a FileManifest for the given project directory.
/// La marca de un fichero que se enumero pero no se pudo leer.
///
/// No puede coincidir con un digest real, porque un digest hexadecimal no
/// lleva letras fuera de `a`..`f`.
const UNREADABLE_CONTENT: &str = "unreadable";

/// The manifest of the tree, from the files the walk found.
///
/// The files are the ones the graph builder walks with, so the manifest
/// describes the graph it is the freshness key of.
pub fn build_manifest(
    project_dir: &Path,
    files: &[TreeFile],
) -> crate::domain::value_objects::file_manifest::FileManifest {
    use crate::domain::value_objects::file_manifest::{FileEntry, FileManifest};
    use sha2::{Digest, Sha256};

    let mut manifest = FileManifest::new(project_dir.to_path_buf());

    for file in files {
        let rel = file
            .path
            .strip_prefix(project_dir)
            .unwrap_or(&file.path)
            .to_path_buf();
        // Content_hash must hash CONTENT, not metadata.
        // The previous mtime proxy produced identical digests for
        // different files written in the same millisecond.
        //
        // A file that cannot be read has no content hash. What is stored
        // then is the mark of not having read it, which no real digest
        // can collide with; the freshness of the snapshot is decided by
        // the real hash, when the file can be read.
        let content_hash = std::fs::read(&file.path)
            .map(|bytes| hex_digest(&Sha256::digest(&bytes)))
            .unwrap_or_else(|_| UNREADABLE_CONTENT.to_string());
        let _ = manifest.entries.insert(
            rel,
            FileEntry {
                content_hash,
                mtime: file.mtime_millis,
                symbol_count: 0,
            },
        );
    }
    manifest
}

/// The digest of some bytes, as the hex text both sides compare.
pub fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace with one caller and one callee, and the temporary root
    /// that holds it and its cache.
    fn workspace_with_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().expect("a temporary root");
        let ws = root.path().join("ws");
        std::fs::create_dir_all(&ws).expect("the workspace is created");
        std::fs::write(
            ws.join("a.rs"),
            "fn callee() -> u32 { 1 }\nfn caller() -> u32 { callee() }\n",
        )
        .expect("the source is written");
        let cache = root.path().join("cache");
        (root, ws, cache)
    }

    #[test]
    fn the_second_call_serves_what_the_first_one_calculated() {
        let (_root, workspace, cache) = workspace_with_root();

        let first = load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            false,
        )
        .expect("the first call answers");
        assert!(
            first.source == GraphSource::Built,
            "the first call had nothing to serve: it calculated the graph"
        );

        // Another session: its own service, its own empty graph, and the
        // same cache.
        let second = load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            false,
        )
        .expect("the second call answers");
        assert!(
            second.source != GraphSource::Built,
            "the second call served the snapshot the first one wrote"
        );
        assert_eq!(
            first.graph.symbol_count(),
            second.graph.symbol_count(),
            "the served graph is the calculated one: the consumer cannot tell \
             them apart"
        );
        assert_eq!(
            first.graph.edge_count(),
            second.graph.edge_count(),
            "the served graph holds the same relations: the consumer cannot \
             tell them apart"
        );
    }

    #[test]
    fn the_forced_call_reads_the_tree_again() {
        let (_root, workspace, cache) = workspace_with_root();

        load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            false,
        )
        .expect("the first call answers");

        let forced = load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            true,
        )
        .expect("the forced call answers");
        assert!(
            forced.source == GraphSource::Built,
            "the caller that asks for the tree again gets the tree read \
             again: the snapshot is not what it asked for"
        );
    }

    #[test]
    fn a_tree_that_changed_is_read_again() {
        let (_root, workspace, cache) = workspace_with_root();

        load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            false,
        )
        .expect("the first call answers");

        std::fs::write(workspace.join("b.rs"), "fn other() -> u32 { 2 }\n")
            .expect("the tree changed");

        let after = load_or_build_graph(
            &AnalysisService::new(),
            &workspace,
            Some(&cache),
            None,
            false,
        )
        .expect("the call answers");
        assert!(
            after.source == GraphSource::Built,
            "the snapshot describes a tree that no longer exists: it is not \
             served"
        );
        assert!(
            after.graph.symbol_count() > 1,
            "the tree was read again, and the new symbol is in the graph"
        );
    }
}
