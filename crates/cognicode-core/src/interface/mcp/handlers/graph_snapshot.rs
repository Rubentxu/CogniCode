//! Durable graph snapshot: the file that lets a restart reuse a graph.

use crate::domain::value_objects::{SNAPSHOT_SCHEMA, SnapshotKey};
use std::path::{Path, PathBuf};

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
pub(super) fn save_durable_snapshot(
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
pub(super) fn tmp_path_for(db_path: &Path) -> PathBuf {
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
pub(super) fn load_durable_snapshot(
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
const MAX_SNAPSHOT_BYTES: u64 = 256 * 1024 * 1024;

pub(super) fn load_durable_snapshot_capped(
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
