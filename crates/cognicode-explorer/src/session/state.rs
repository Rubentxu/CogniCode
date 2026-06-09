//! Per-session state model.
//!
//! Each `BrainSessionState` is owned by a single
//! [`crate::session::service::BrainSessionService`] and lives behind
//! a `Mutex` so the per-session logic can take a snapshot for status
//! dumps without holding the registry lock. The registry itself only
//! touches the inner service via the `Arc` it hands out — it never
//! reaches inside the state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default time-to-live for a freshly opened session, in seconds.
/// `1800s = 30 minutes`. The registry's `evict_expired` uses this
/// default when the caller omits `ttl`. `ttl = 0` disables expiry.
pub const DEFAULT_TTL_SECS: u64 = 1800;

/// Default maximum number of [`HistoryEntry`] rows retained per
/// session. Once the cap is hit, the oldest entry is dropped FIFO on
/// every successful push.
pub const DEFAULT_HISTORY_CAP: usize = 50;

/// One row in a session's history. Captures the question the user
/// asked, a short summary of the answer that came back, the router
/// pattern id (mirrors [`crate::ask::patterns::QuestionCategory`]
/// discriminant), and the UTC timestamp of the ask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub question: String,
    pub answer_summary: String,
    /// Discriminant of [`crate::ask::patterns::QuestionCategory`]
    /// (0..=7). Stored as a number on the wire so the schema stays
    /// stable even if new variants are added.
    pub pattern_id: u8,
    pub ts: DateTime<Utc>,
}

/// Per-session state. The struct is plain data plus a `new`
/// constructor; all mutations happen through the surrounding
/// `BrainSessionService`, which takes the lock for the duration of
/// the operation.
///
/// `history` is `#[serde(default)]` so an empty vector serializes as
/// `[]` (not `null`, not omitted). That keeps the wire-level
/// contract stable for `brain_status` consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSessionState {
    pub session_id: String,
    pub workspace_id: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    /// Time-to-live in seconds. `0` disables expiry entirely. Any
    /// other value is compared against `now - last_activity` on
    /// `brain_open` / `brain_attach` and the session is lazily
    /// evicted when the gap exceeds it.
    pub ttl: u64,
    /// Optional "focus node" — the symbol id the user is currently
    /// exploring. `brain_ask` prepends a backtick-quoted copy of this
    /// value to the question before it reaches the ask router.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_node: Option<String>,
    /// Bounded ring of past asks. The service pushes the newest entry
    /// at the tail and truncates the head to stay within
    /// [`DEFAULT_HISTORY_CAP`].
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

impl BrainSessionState {
    /// Construct a fresh state with `created_at = last_activity = now`.
    /// `history` starts empty.
    pub fn new(session_id: String, workspace_id: String, ttl: u64) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            workspace_id,
            created_at: now,
            last_activity: now,
            ttl,
            focus_node: None,
            history: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip a fresh state and assert the wire shape: `history`
    /// is `[]` (NOT `null` or omitted), `session_id` roundtrips
    /// intact, and `focus_node` is `null` when absent.
    #[test]
    fn brain_session_state_serializes_with_uuid_and_history() {
        let state = BrainSessionState::new(
            "00000000-0000-4000-8000-000000000001".to_string(),
            "ws-alpha".to_string(),
            DEFAULT_TTL_SECS,
        );

        let json = serde_json::to_string(&state).expect("serialize");
        // Empty history MUST render as `[]` so consumers never have
        // to special-case null/missing.
        assert!(
            json.contains("\"history\":[]"),
            "history must serialize as [], got: {json}"
        );
        // The session id must roundtrip verbatim.
        assert!(
            json.contains("\"session_id\":\"00000000-0000-4000-8000-000000000001\""),
            "session_id lost in serialization: {json}"
        );
        // focus_node is None; serde(default, skip_serializing_if) makes
        // it disappear from the payload entirely.
        assert!(
            !json.contains("focus_node"),
            "focus_node must be omitted when None, got: {json}"
        );

        // Roundtrip back and check the field values survive.
        let back: BrainSessionState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.session_id, state.session_id);
        assert_eq!(back.workspace_id, state.workspace_id);
        assert_eq!(back.ttl, DEFAULT_TTL_SECS);
        assert_eq!(back.history.len(), 0);
        assert!(back.focus_node.is_none());
    }
}
