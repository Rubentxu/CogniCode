//! Brain Session — conversational graph exploration.
//!
//! Adds six MCP tools (`brain_open`, `brain_attach`, `brain_ask`,
//! `brain_focus`, `brain_status`, `brain_close`) backed by an
//! in-memory `SessionRegistry` keyed by opaque session ids. Each
//! session owns a `BrainSessionService` that holds:
//!
//! - the per-session state ([`state::BrainSessionState`]),
//! - ISP-segregated service facades ([`SearchService`],
//!   [`ViewService`], [`WorkspaceService`]),
//! - the optional call graph (used by follow-up asks).
//!
//! Lock protocol: `SessionRegistry` holds a `Mutex<HashMap<_, _>>` only
//! for the duration of map operations. The `Arc<BrainSessionService>`
//! is cloned out and the lock is released BEFORE any `await`. The
//! per-session [`state::BrainSessionState`] sits behind its own
//! `Mutex` inside the service, also acquired and released without
//! crossing `await`.
//!
//! Public surface:
//! - [`state::BrainSessionState`] — owned per-session data.
//! - [`state::HistoryEntry`] — one row in the cap-50 history.
//! - [`service::BrainSessionService`] — per-session logic (focus,
//!   history, ask).
//! - [`registry::SessionRegistry`] — process-wide registry with
//!   TTL-based lazy eviction.

pub mod registry;
pub mod service;
pub mod state;

pub use registry::SessionRegistry;
pub use service::BrainSessionService;
pub use state::{BrainSessionState, DEFAULT_HISTORY_CAP, DEFAULT_TTL_SECS, HistoryEntry};
