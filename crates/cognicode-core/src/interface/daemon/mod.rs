//! The daemon's local socket: how a client asks whether the index is
//! still fresh.
//!
//! ## What travels
//!
//! One JSON line each way. The request says what it wants (`{"op":"status"}`)
//! and the answer says whether the daemon's index still describes the tree
//! and which tree state it is about (the digest of its manifest).
//!
//! ## Why a socket and not the HTTP server
//!
//! The HTTP server is the daemon's public face: it needs a port, and a
//! port is something a network can reach. The question asked here is
//! local —"is the index you keep still fresh?"— and it is asked by
//! processes on the same machine, so it travels over a unix socket inside
//! the user's cache, with 0600, next to the snapshot the two of them
//! already share. Nothing about it is reachable from another machine.
//!
//! ## The deadline
//!
//! The client asks with a deadline. A daemon that hangs must not hang the
//! client: the answer is an optimisation, and a client that waits forever
//! for one has turned an optimisation into a dependency.
//!
//! ## The daemon's word
//!
//! The daemon says "fresh" while its watcher has seen no change since it
//! calculated the index. Its word is about a tree state —the digest of
//! the manifest— so a client can tell whether the word is about the
//! manifest it holds. A word about another tree is not evidence of
//! anything, and the client checks before it believes it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::application::freshness::FreshnessOracle;
use crate::application::graph_cache;
use crate::application::graph_snapshot;
use crate::domain::value_objects::file_manifest::FileManifest;

/// How long a client waits for the daemon's answer.
///
/// The answer is an optimisation: a client that cannot get it goes on
/// without it, so the wait is short and bounded.
const CLIENT_DEADLINE: Duration = Duration::from_millis(500);

/// What the daemon says about the index it keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Whether the daemon believes its index still describes the tree.
    pub fresh: bool,
    /// The digest of the manifest of the index the daemon keeps.
    ///
    /// Empty when the daemon keeps no index: there is nothing to vouch
    /// for.
    pub manifest_digest: String,
}

/// What the client asks for.
#[derive(Debug, Serialize, Deserialize)]
struct StatusRequest {
    op: String,
}

/// The index the daemon keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRecord {
    /// The digest of the manifest of the index.
    pub manifest_digest: String,
    /// The symbols the index holds.
    pub symbols: usize,
    /// The relations the index holds.
    pub relationships: usize,
}

/// The daemon's own state: what it knows about the index it keeps.
///
/// The state is shared between the socket server and whoever rebuilds
/// the index, because the answer and the rebuild have to agree: a daemon
/// that says "fresh" while it is rebuilding would be lying for the
/// length of the rebuild.
#[derive(Debug, Default)]
pub struct DaemonState {
    index: Mutex<Option<IndexRecord>>,
    changed: AtomicBool,
}

impl DaemonState {
    /// A daemon that keeps no index yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The index the daemon keeps, freshly calculated.
    pub fn publish(&self, record: IndexRecord) {
        if let Ok(mut index) = self.index.lock() {
            *index = Some(record);
        }
        self.changed.store(false, Ordering::Release);
    }

    /// Something changed: the index is not fresh until it is calculated
    /// again.
    ///
    /// The watcher says this the moment it sees the change, not when the
    /// rebuild finishes: a daemon that keeps saying "fresh" while the tree
    /// is moving is a daemon that cannot be believed.
    pub fn mark_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }

    /// What the daemon answers with.
    pub fn status(&self) -> DaemonStatus {
        let changed = self.changed.load(Ordering::Acquire);
        let index = self.index.lock().ok().and_then(|index| index.clone());
        match index {
            Some(index) if !changed => DaemonStatus {
                fresh: true,
                manifest_digest: index.manifest_digest,
            },
            _ => DaemonStatus {
                fresh: false,
                manifest_digest: String::new(),
            },
        }
    }
}

/// The index the daemon keeps, read from the snapshot it wrote.
///
/// The snapshot is the product's own state and it is what a client will
/// be served, so the index the daemon vouches for is the one the snapshot
/// describes. A daemon with no snapshot has no index: there is nothing to
/// vouch for.
pub fn index_from_snapshot(cache_root: &Path, workspace: &Path) -> Option<IndexRecord> {
    let path = graph_cache::snapshot_path(cache_root, workspace);
    let (_key, graph, manifest) = graph_snapshot::load_durable_snapshot(&path)?;
    Some(IndexRecord {
        manifest_digest: graph_snapshot::manifest_digest(&manifest),
        symbols: graph.symbol_count(),
        relationships: graph.edge_count(),
    })
}

/// Binds the daemon's socket and makes it private to the user.
///
/// The socket is the product's own state in the user's cache, and it is
/// a door into a running daemon: only the user opens it.
pub fn bind(socket: &Path) -> std::io::Result<UnixListener> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A socket left behind by a daemon that died would keep the new one
    // from binding. The file is not evidence of a live daemon: it is
    // removed and the new daemon takes the name.
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Serves the daemon's status until the process ends.
///
/// Each question is answered on its own thread: a client that hangs must
/// not take the daemon's answers away from the other clients.
pub fn serve(listener: UnixListener, state: Arc<DaemonState>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let state = state.clone();
        std::thread::spawn(move || {
            let _ = answer(stream, &state);
        });
    }
}

/// Answers one question.
fn answer(mut stream: UnixStream, state: &DaemonState) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let status = match serde_json::from_str::<StatusRequest>(&line) {
        Ok(request) if request.op == "status" => state.status(),
        // A question the daemon does not know is not a question it
        // answers with a claim: it answers "no".
        _ => DaemonStatus {
            fresh: false,
            manifest_digest: String::new(),
        },
    };

    stream.write_all(serde_json::to_string(&status)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// Asks the daemon whether its index is fresh, and about which tree.
///
/// `None` when there is no daemon to answer: no socket, no answer, or an
/// answer that is not one. The client then goes on as it always did.
pub fn ask(socket: &Path) -> Option<DaemonStatus> {
    let mut stream = UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(CLIENT_DEADLINE)).ok()?;
    stream.set_write_timeout(Some(CLIENT_DEADLINE)).ok()?;
    stream.write_all(b"{\"op\":\"status\"}\n").ok()?;
    stream.flush().ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// The client that asks a daemon whether its index is still fresh.
///
/// This is the adapter of the application's port: it knows how the answer
/// travels, and the application never learns.
pub struct DaemonOracle {
    socket: PathBuf,
}

impl DaemonOracle {
    /// The oracle of the daemon that serves `workspace`.
    pub fn for_workspace(cache_root: &Path, workspace: &Path) -> Self {
        Self {
            socket: graph_cache::daemon_socket_path(cache_root, workspace),
        }
    }

    /// The oracle that asks the daemon listening at `socket`.
    pub fn at(socket: PathBuf) -> Self {
        Self { socket }
    }
}

impl FreshnessOracle for DaemonOracle {
    fn vouches_for(&self, manifest: &FileManifest) -> bool {
        let Some(status) = ask(&self.socket) else {
            return false;
        };
        // The word is about a tree state: it only counts when it is about
        // THIS manifest. A daemon that is watching another build says
        // nothing about this one.
        status.fresh && status.manifest_digest == graph_snapshot::manifest_digest(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_without_an_index_says_nothing_is_fresh() {
        let state = DaemonState::new();
        let status = state.status();
        assert!(!status.fresh, "no index, no claim");
        assert!(
            status.manifest_digest.is_empty(),
            "there is nothing to vouch for"
        );
    }

    #[test]
    fn a_change_makes_the_index_not_fresh_until_it_is_calculated_again() {
        let state = DaemonState::new();
        state.publish(IndexRecord {
            manifest_digest: "abc".to_string(),
            symbols: 2,
            relationships: 1,
        });
        assert!(state.status().fresh, "the index was just calculated");

        state.mark_changed();
        let status = state.status();
        assert!(!status.fresh, "the tree moved: the index is not fresh");
        assert!(
            status.manifest_digest.is_empty(),
            "a stale index vouches for nothing"
        );

        state.publish(IndexRecord {
            manifest_digest: "def".to_string(),
            symbols: 3,
            relationships: 2,
        });
        assert!(state.status().fresh, "the index was calculated again");
        assert_eq!(state.status().manifest_digest, "def");
    }
}
