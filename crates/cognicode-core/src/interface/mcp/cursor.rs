//! H2.3 — Typed cursor errors and anchored pagination for the MCP
//! `list_tools` and `resources/list` surfaces.
//!
//! H2.3 spec: "Reutilizar cursor/paginación existentes; anclar
//! snapshot, workspace, filtros y orden. Error tipado para cursor
//! manipulado o caducado. Formato compacto por defecto sin perder
//! causas."
//!
//! The previous cursor was a bare base64-encoded numeric offset
//! (e.g. `base64("20")`). The new cursor is a compact base64-JSON
//! envelope `{v, ws, snap, off}` where `ws` and `snap` are short
//! deterministic hashes of the workspace path and the snapshot
//! digest. A cursor minted for one context (workspace or
//! snapshot) and replayed in a different context is rejected
//! with a typed error. Bare-numeric legacy cursors are typed as
//! `Garbled` — never silently reset to page 0.
//!
//! Compactness: the envelope is ~80 bytes JSON, base64-encoded to
//! ~108 bytes. The wire shape stays well below any practical
//! limit; no fields are dropped.
//!
//! The typed error is the contract. A consumer routing on
//! `kind` (snake_case string) is safe to add new variants on
//! purpose; an unmapped kind is a wire-protocol break the
//! consumer sees as a JSON parse error, not a silent pass.
//!
//! Filter and order anchoring are reserved for a future cycle:
//! H2.3 cycle 1 wires the snapshot and workspace anchors that
//! already have a use today. Filters/order will land with
//! `parameters_aliases` when a list surface actually exposes
//! them, with the same typed-error pattern.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

const CURSOR_VERSION: u32 = 1;

/// Hash a string to a 16-hex-char short form. Deterministic
/// (SipHash-1-3 via `DefaultHasher`), no allocation, no external
/// dependency. Collisions are not a security property here —
/// the cursor is integrity-tag, not identifier. A collision
/// would be caught as a same-context match, which is the right
/// answer (the cursor is honestly compatible).
fn short_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    let bytes = h.finish().to_le_bytes();
    let mut out = String::with_capacity(16);
    for b in &bytes[..8] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// The context the cursor is anchored to. Both fields are
/// short-hashed into the envelope; the original strings are NOT
/// preserved in the wire shape (compactness).
#[derive(Debug, Clone)]
pub struct CursorContext {
    /// Workspace path (or `__tools__` for the global catalog).
    pub workspace: String,
    /// Snapshot digest (catalog digest for `tools/list`; can be
    /// a workspace-state digest for `resources/list`).
    pub snapshot: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorEnvelope {
    v: u32,
    ws: String,
    snap: String,
    off: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorErrorKind {
    /// The cursor is not a valid envelope (bad base64, bad JSON,
    /// wrong version, bare-numeric legacy cursor).
    Garbled,
    /// The cursor's workspace hash does not match the current
    /// request's workspace.
    WorkspaceMismatch,
    /// The cursor's snapshot hash does not match the current
    /// snapshot digest.
    SnapshotExpired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorError {
    pub kind: CursorErrorKind,
    pub reason: String,
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.reason)
    }
}

impl std::error::Error for CursorError {}

/// Encode a cursor for the given context and offset. The result
/// is a base64 string of a small JSON envelope; it is compact
/// (≈100 bytes) and the fields are hashed so the original
/// workspace/snapshot strings do not leak into the wire shape.
pub fn encode_cursor(ctx: &CursorContext, offset: usize) -> String {
    encode_envelope(CursorEnvelope {
        v: CURSOR_VERSION,
        ws: short_hash(&ctx.workspace),
        snap: short_hash(&ctx.snapshot),
        off: offset,
    })
}

fn encode_envelope(env: CursorEnvelope) -> String {
    let json = serde_json::to_vec(&env).expect("envelope is always serializable");
    base64::engine::general_purpose::STANDARD.encode(json)
}

/// Verify a cursor against the current context. Returns the
/// offset on success, or a typed `CursorError` on failure. The
/// cause is preserved verbatim in `reason` ("sin perder
/// causas").
pub fn verify_cursor(cursor: &str, ctx: &CursorContext) -> Result<usize, CursorError> {
    // Decode base64
    let json_bytes = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .map_err(|e| CursorError {
            kind: CursorErrorKind::Garbled,
            reason: format!(
                "cursor is not valid base64: {e}; legacy bare-number cursors are no longer accepted"
            ),
        })?;
    // Parse JSON envelope (rejects bare numbers, which are not
    // valid JSON envelopes — a typed Garbled error, never a
    // silent reset).
    let env: CursorEnvelope = serde_json::from_slice(&json_bytes).map_err(|e| CursorError {
        kind: CursorErrorKind::Garbled,
        reason: format!(
            "cursor is not a valid v{CURSOR_VERSION} envelope: {e}; legacy bare-number cursors are no longer accepted"
        ),
    })?;
    if env.v != CURSOR_VERSION {
        return Err(CursorError {
            kind: CursorErrorKind::Garbled,
            reason: format!(
                "cursor version {} is not supported; this server speaks v{CURSOR_VERSION}",
                env.v
            ),
        });
    }
    let ws_expected = short_hash(&ctx.workspace);
    if env.ws != ws_expected {
        return Err(CursorError {
            kind: CursorErrorKind::WorkspaceMismatch,
            reason: format!(
                "cursor was minted for a different workspace; expected hash {ws_expected} for current request"
            ),
        });
    }
    let snap_expected = short_hash(&ctx.snapshot);
    if env.snap != snap_expected {
        return Err(CursorError {
            kind: CursorErrorKind::SnapshotExpired,
            reason: format!(
                "cursor was minted for snapshot hash {}; current snapshot is {snap_expected}; the cursor is expired",
                env.snap
            ),
        });
    }
    Ok(env.off)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let ctx = CursorContext {
            workspace: "/repo/a".to_string(),
            snapshot: "snap-1".to_string(),
        };
        let c = encode_cursor(&ctx, 40);
        assert_eq!(verify_cursor(&c, &ctx), Ok(40));
    }

    #[test]
    fn workspace_mismatch_is_typed() {
        let a = CursorContext {
            workspace: "/a".to_string(),
            snapshot: "s".to_string(),
        };
        let b = CursorContext {
            workspace: "/b".to_string(),
            snapshot: "s".to_string(),
        };
        let c = encode_cursor(&a, 0);
        let err = verify_cursor(&c, &b).unwrap_err();
        assert_eq!(err.kind, CursorErrorKind::WorkspaceMismatch);
    }

    #[test]
    fn snapshot_change_is_typed() {
        let old = CursorContext {
            workspace: "/a".to_string(),
            snapshot: "old".to_string(),
        };
        let new = CursorContext {
            workspace: "/a".to_string(),
            snapshot: "new".to_string(),
        };
        let c = encode_cursor(&old, 0);
        let err = verify_cursor(&c, &new).unwrap_err();
        assert_eq!(err.kind, CursorErrorKind::SnapshotExpired);
    }

    #[test]
    fn legacy_bare_number_is_garbled() {
        let ctx = CursorContext {
            workspace: "/a".to_string(),
            snapshot: "s".to_string(),
        };
        let legacy = base64::engine::general_purpose::STANDARD.encode("20");
        let err = verify_cursor(&legacy, &ctx).unwrap_err();
        assert_eq!(err.kind, CursorErrorKind::Garbled);
    }
}
