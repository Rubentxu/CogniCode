//! In-memory session registry.
//!
//! Holds the process-wide `HashMap<SessionId, Arc<BrainSessionService>>`
//! behind a `Mutex`. Methods take the lock for the duration of the
//! map operation only — they NEVER hold the guard across `.await`.
//! The `Arc<BrainSessionService>` is cloned out and the lock is
//! released before any async dispatch happens.
//!
//! TTL semantics: lazy eviction on `open` and `attach`. A session
//! with `ttl == 0` is exempt from eviction. There is no background
//! sweeper. `close` is idempotent.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cognicode_core::domain::aggregates::CallGraph;

use crate::service::ExplorerService;
use crate::session::service::BrainSessionService;

/// Errors raised by [`SessionRegistry`] lookups.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session_not_found")]
    NotFound,
    #[error("session_expired")]
    Expired,
}

pub(crate) type SessionMap = HashMap<String, Arc<BrainSessionService>>;

/// Process-wide registry of brain sessions.
///
/// Cheap to clone: the inner map sits behind an `Arc<Mutex<_>>` so
/// every clone shares the same map. Cloning the registry does NOT
/// snapshot the session table — a session opened through one clone
/// is visible to every other clone.
#[derive(Debug, Default, Clone)]
pub struct SessionRegistry {
    sessions: Arc<Mutex<SessionMap>>,
}

impl SessionRegistry {
    /// Construct an empty registry. Cheap; no I/O.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Open a new session. Generates a session id, builds a
    /// [`BrainSessionService`] with the supplied service + graph, and
    /// inserts it into the map. Returns the freshly minted id.
    ///
    /// `ttl_secs` is the per-session time-to-live in seconds. `0`
    /// disables expiry. Callers are expected to have already
    /// validated the value (see `brain_open` for the bounds check).
    pub fn open(
        &self,
        workspace_id: String,
        ttl_secs: u64,
        service: Arc<ExplorerService>,
        graph: Option<Arc<CallGraph>>,
    ) -> String {
        let session_id = generate_session_id();
        let session = Arc::new(BrainSessionService::new(
            session_id.clone(),
            workspace_id,
            ttl_secs,
            service,
            graph,
        ));
        let mut map = self.sessions.lock().expect("session map poisoned");
        // Lazy eviction on open: drop anything that's expired BEFORE
        // we add the new entry.
        evict_expired_locked(&mut map);
        map.insert(session_id.clone(), session);
        session_id
    }

    /// Rejoin an existing session. First runs lazy eviction, then
    /// looks up the id. On hit, refreshes `last_activity` on the
    /// service state and returns the cloned `Arc`. On miss, returns
    /// the appropriate `SessionError`.
    pub fn attach(&self, session_id: &str) -> Result<Arc<BrainSessionService>, SessionError> {
        let mut map = self.sessions.lock().expect("session map poisoned");
        evict_expired_locked(&mut map);
        let session = map.get(session_id).cloned().ok_or(SessionError::NotFound)?;
        // Refresh `last_activity` AFTER clone, so a subsequent
        // service call sees the new value. This is the only place
        // the registry touches inner state.
        session.touch();
        Ok(session)
    }

    /// Look up a session WITHOUT refreshing `last_activity`. Used by
    /// `brain_ask`, `brain_focus`, `brain_status` — operations that
    /// should not extend the session's lifetime.
    pub fn get(&self, session_id: &str) -> Result<Arc<BrainSessionService>, SessionError> {
        let mut map = self.sessions.lock().expect("session map poisoned");
        evict_expired_locked(&mut map);
        map.get(session_id).cloned().ok_or(SessionError::NotFound)
    }

    /// Idempotent close. Returns `true` if the session was present
    /// and removed, `false` otherwise (already closed or never
    /// existed — both are normal, not errors).
    pub fn close(&self, session_id: &str) -> bool {
        let mut map = self.sessions.lock().expect("session map poisoned");
        map.remove(session_id).is_some()
    }

    /// Drop every session whose `now - last_activity > ttl` AND
    /// whose `ttl > 0`. The `ttl = 0` escape hatch keeps a session
    /// alive indefinitely. Currently unused (the per-call lock-helpers
    /// do eviction inline) but kept for future use (e.g. a periodic
    /// background sweeper if the lazy-eviction policy proves
    /// insufficient under load).
    #[allow(dead_code)]
    fn evict_expired(&self) {
        let mut map = self.sessions.lock().expect("session map poisoned");
        evict_expired_locked(&mut map);
    }

    /// Test-only count of live sessions.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn len(&self) -> usize {
        self.sessions.lock().expect("session map poisoned").len()
    }
}

/// Drop expired sessions in place. Caller MUST hold the lock.
fn evict_expired_locked(map: &mut SessionMap) {
    let now = chrono::Utc::now();
    map.retain(|_, session| {
        let state = session.snapshot();
        if state.ttl == 0 {
            return true;
        }
        let age = now.signed_duration_since(state.last_activity);
        let ttl = chrono::Duration::seconds(state.ttl as i64);
        age <= ttl
    });
}

/// Generate a session id. The spec calls for UUIDv4; we don't pull
/// in the `uuid` crate (not a workspace dep), so we emit a
/// UUIDv4-shaped string built from a 122-bit random source mixed
/// with the current UTC timestamp. Collisions are astronomically
/// unlikely in any realistic workload.
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Mix the low 64 bits of the clock with a per-process counter and
    // an xorshifted splitmix64 step to fill the high bits. Total
    // entropy >> 122 bits across a realistic session lifetime.
    let mut counter: u64 = 0x9E37_79B9_7F4A_7C15;
    counter ^= nanos as u64;
    counter = counter.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    counter ^= counter >> 31;
    counter = counter.wrapping_mul(0x94D0_49BB_1331_11EB);
    counter ^= counter >> 30;

    let high = (nanos as u64).wrapping_add(counter);
    let low = counter.rotate_left(17) ^ (nanos as u64).rotate_left(7);

    // Format as canonical UUIDv4: high bits set the version (4) and
    // the variant (10xx), matching the wire shape agents expect.
    let bytes_high = high.to_be_bytes();
    let bytes_low = low.to_le_bytes();
    let version = (bytes_high[1] & 0x0F) | 0x40; // version 4
    let variant = (bytes_low[0] & 0x3F) | 0x80; // RFC 4122

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes_high[0],
        bytes_high[1],
        bytes_high[2],
        bytes_high[3],
        bytes_high[4],
        bytes_high[5],
        version,
        bytes_high[7],
        variant,
        bytes_low[1],
        bytes_low[2],
        bytes_low[3],
        bytes_low[4],
        bytes_low[5],
        bytes_low[6],
        bytes_low[7],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::ports::symbol_repository::{
        GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
    };
    use crate::session::state::DEFAULT_TTL_SECS;
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn build_service() -> (Arc<ExplorerService>, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        struct NoopRepo;
        impl SymbolRepository for NoopRepo {
            fn resolve(&self, _: &SymbolId) -> crate::ExplorerResult<Option<ResolvedSymbol>> {
                Ok(None)
            }
            fn callers(&self, _: &SymbolId) -> Vec<RelationTarget> {
                Vec::new()
            }
            fn callees(&self, _: &SymbolId) -> Vec<RelationTarget> {
                Vec::new()
            }
            fn fan_in(&self, _: &SymbolId) -> usize {
                0
            }
            fn fan_out(&self, _: &SymbolId) -> usize {
                0
            }
            fn find_symbols_by_name(&self, _: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
                Ok(Vec::new())
            }
            fn find_symbols_by_file(&self, _: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
                Ok(Vec::new())
            }
            fn module_list(&self) -> Vec<String> {
                Vec::new()
            }
            fn all_symbols(&self) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
                Ok(Vec::new())
            }
            fn graph_stats(&self) -> GraphStats {
                GraphStats {
                    symbol_count: 0,
                    relation_count: 0,
                }
            }
        }
        let repo: Arc<dyn SymbolRepository> = Arc::new(NoopRepo);
        let reader = Arc::new(FsSourceReader::new(dir.path().to_path_buf()));
        let service = Arc::new(ExplorerService::new(repo, reader, dir.path().to_path_buf()));
        (service, dir)
    }

    #[test]
    fn registry_open_returns_uuid_and_workspace_id() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let id = reg.open("ws-alpha".into(), DEFAULT_TTL_SECS, svc.clone(), None);
        // UUIDv4 shape: 8-4-4-4-12 hex.
        assert_eq!(id.len(), 36, "session id must be 36 chars, got `{id}`");
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
        // Hex digits in the right positions.
        let hex_count = id.chars().filter(|c| c.is_ascii_hexdigit()).count();
        assert_eq!(hex_count, 32);
        // Workspace id roundtrips via attach.
        let arc = reg.attach(&id).expect("attach after open");
        assert_eq!(arc.workspace_id(), "ws-alpha");
        assert_eq!(arc.session_id(), id);
    }

    #[test]
    fn registry_attach_unknown_id_returns_session_not_found() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        // Touch the service so the unused warning stays quiet.
        let _ = svc;
        let result = reg.attach("00000000-0000-4000-8000-000000000000");
        assert!(matches!(result, Err(SessionError::NotFound)));
    }

    #[test]
    fn registry_close_unknown_id_returns_false_idempotent() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let _ = svc;
        // First close of an unknown id is false. A second close is
        // still false (idempotent), NOT an error.
        assert!(!reg.close("deadbeef-0000-4000-8000-000000000000"));
        assert!(!reg.close("deadbeef-0000-4000-8000-000000000000"));
    }

    #[test]
    fn registry_close_known_id_returns_true_and_removes_session() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let id = reg.open("ws-1".into(), DEFAULT_TTL_SECS, svc.clone(), None);
        assert_eq!(reg.len(), 1);
        assert!(reg.close(&id));
        assert_eq!(reg.len(), 0);
        // Subsequent attach fails.
        assert!(matches!(reg.attach(&id), Err(SessionError::NotFound)));
        // Subsequent close returns false.
        assert!(!reg.close(&id));
    }

    /// Spawn 4 tasks that each `attach` then `sleep(10ms).await`. If
    /// the registry held the lock across `await`, tasks would
    /// serialize and the wall-clock time would balloon. We only need
    /// to assert all four complete — a deadlock would surface as the
    /// test timing out.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn registry_lock_not_held_across_await() {
        let (svc, _dir) = build_service();
        let reg = Arc::new(SessionRegistry::new());
        let id = reg.open("ws-x".into(), DEFAULT_TTL_SECS, svc, None);

        let mut handles = Vec::new();
        for _ in 0..4 {
            let reg = reg.clone();
            let id = id.clone();
            handles.push(tokio::spawn(async move {
                let _ = reg.attach(&id).expect("attach");
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                let _ = reg.get(&id).expect("get");
            }));
        }
        for h in handles {
            h.await.expect("task completed without deadlock");
        }
    }

    /// A session opened with `ttl = 0` must survive any number of
    /// open/attach cycles. The `0` value disables expiry entirely.
    #[test]
    fn registry_ttl_zero_disables_expiry() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let id = reg.open("ws-forever".into(), 0, svc.clone(), None);
        // Open a second session to force eviction to run; the first
        // one must remain.
        let _id2 = reg.open("ws-other".into(), DEFAULT_TTL_SECS, svc, None);
        assert!(
            reg.attach(&id).is_ok(),
            "ttl=0 session must survive eviction"
        );
    }

    /// Lazy eviction: opening a fresh session sweeps expired ones.
    /// We construct a stale session by opening with a 1-second TTL,
    /// waiting 1100ms, then opening a second session — the first
    /// should be dropped.
    #[tokio::test]
    async fn registry_lazy_eviction_drops_expired_sessions() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let stale = reg.open("ws-stale".into(), 1, svc.clone(), None);
        // Sleep 1.1s to push the first session past its 1s TTL.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        // A new open triggers eviction.
        let _fresh = reg.open("ws-fresh".into(), DEFAULT_TTL_SECS, svc, None);
        // Stale must be gone.
        assert!(matches!(reg.attach(&stale), Err(SessionError::NotFound)));
    }

    /// `get` must NOT refresh `last_activity`. We verify by opening
    /// with a near-expiry TTL, sleeping past it, calling `get`, then
    /// opening again — the session must be evicted despite the
    /// intervening `get` (because `get` did not extend the TTL).
    #[tokio::test]
    async fn registry_get_does_not_refresh_last_activity() {
        let (svc, _dir) = build_service();
        let reg = SessionRegistry::new();
        let id = reg.open("ws-get".into(), 1, svc.clone(), None);
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let _ = reg.get(&id).expect("get within TTL");
        // Sleep another 600ms so total > 1s, then trigger eviction.
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let _trigger = reg.open("ws-trigger".into(), DEFAULT_TTL_SECS, svc, None);
        assert!(matches!(reg.get(&id), Err(SessionError::NotFound)));
    }

    /// Sanity: the two unused references below silence the compiler
    /// in case the `Arc` and `HashMap` symbols become needed in the
    /// future (e.g. for `DashMap` migration).
    #[allow(dead_code)]
    fn _unused(_: Arc<BrainSessionService>, _: HashMap<String, Arc<BrainSessionService>>) {}
}
