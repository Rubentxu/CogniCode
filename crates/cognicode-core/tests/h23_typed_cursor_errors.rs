//! H2.3 (delivery-first-radical, package integration) — typed
//! cursor errors + anchored snapshot/workspace in `tools/list` and
//! `resources/list`.
//!
//! H2.3 spec: "Reutilizar cursor/paginación existentes; anclar
//! snapshot, workspace, filtros y orden. Error tipado para cursor
//! manipulado o caducado. Formato compacto por defecto sin perder
//! causas."
//!
//! The current pagination surface (`list_tools` in
//! `rmcp_adapter.rs`, `handle_resources_list` in `resources.rs`)
//! silently resets to offset 0 on a malformed cursor
//! (`.unwrap_or(0)`). That is the defect H2.3 fixes: a tampered
//! or expired cursor must surface as a TYPED error, not a silent
//! page-1.
//!
//! The new cursor is a compact base64-JSON envelope
//! `{v:1, ws, snap, off}` where:
//!   - `ws`    is a short hash of the workspace path (or
//!             `__tools__` for `tools/list`, which is global).
//!   - `snap`  is a short hash of the snapshot (catalog digest for
//!             `tools/list`; workspace state for `resources/list`).
//!   - `off`   is the byte/row offset within that snapshot.
//!
//! A bare-numeric legacy cursor (the previous format) is now a
//! `Garbled` typed error. The typed error is the contract: the
//! consumer can route on `kind` and the cause is preserved
//! verbatim in `reason` ("formato compacto por defecto sin perder
//! causas").
//!
//! This test exercises the cursor module's encode/decode/verify
//! surface directly. The integration in `list_tools` /
//! `handle_resources_list` is the production change that turns
//! the typed error into a JSON-RPC error frame.

use base64::Engine;
use cognicode_core::interface::mcp::cursor::{
    CursorContext, CursorErrorKind, encode_cursor, verify_cursor,
};
use serde_json::Value;

#[test]
fn round_trip_preserves_offset() {
    // The compact format must round-trip: encode({off: 40}) and
    // decode in the same context gives Some(40). No drift, no
    // hidden state.
    let ctx = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    let cursor = encode_cursor(&ctx, 40);
    assert_eq!(
        verify_cursor(&cursor, &ctx),
        Ok(40),
        "round-trip must preserve the offset"
    );
}

#[test]
fn garbled_cursor_returns_typed_garbled_error() {
    // The previous format was a bare base64 number. The new
    // format is a versioned envelope. A bare number is now a
    // tampered cursor and must surface as `Garbled` — NOT a
    // silent reset to page 0.
    let ctx = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    // Encode a bare number (the legacy format) and feed it back.
    let legacy = base64::engine::general_purpose::STANDARD.encode("20");
    let result = verify_cursor(&legacy, &ctx);
    let err = result.expect_err("legacy bare-number cursor must NOT be accepted");
    assert_eq!(
        err.kind,
        CursorErrorKind::Garbled,
        "legacy cursors are a typed Garbled error, not a silent reset"
    );
    assert!(
        err.reason.contains("legacy") || err.reason.contains("version"),
        "the reason must preserve the cause: got {:?}",
        err.reason
    );
}

#[test]
fn non_base64_cursor_returns_typed_garbled_error() {
    let ctx = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    let err = verify_cursor("not-base64-!!!", &ctx).expect_err("garbage must reject");
    assert_eq!(err.kind, CursorErrorKind::Garbled);
}

#[test]
fn cursor_from_different_workspace_is_workspace_mismatch() {
    // Anchor: snapshot/workspace. A cursor made for workspace A
    // handed to workspace B is a typed WorkspaceMismatch — the
    // server does not silently accept a page from a different
    // workspace's catalog.
    let ctx_a = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    let ctx_b = CursorContext {
        workspace: "/repo/b".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    let cursor = encode_cursor(&ctx_a, 20);
    let err = verify_cursor(&cursor, &ctx_b).expect_err("workspace mismatch must reject");
    assert_eq!(err.kind, CursorErrorKind::WorkspaceMismatch);
}

#[test]
fn cursor_after_snapshot_change_is_snapshot_expired() {
    // Anchor: snapshot. The catalog (or workspace state) was
    // snap-abc when the cursor was minted; the next request sees
    // snap-xyz. The cursor is expired and must surface as typed
    // SnapshotExpired — not silently resume from a stale
    // position.
    let ctx_old = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-abc".to_string(),
    };
    let ctx_new = CursorContext {
        workspace: "/repo/a".to_string(),
        snapshot: "snap-xyz".to_string(),
    };
    let cursor = encode_cursor(&ctx_old, 40);
    let err = verify_cursor(&cursor, &ctx_new).expect_err("snapshot change must reject");
    assert_eq!(err.kind, CursorErrorKind::SnapshotExpired);
}

#[test]
fn cursor_serializes_to_typed_json_for_wire_response() {
    // The error returned to the consumer must be a typed JSON
    // document with a stable `kind` field and a `reason` that
    // preserves the cause. This is the "sin perder causas" half
    // of the spec.
    use cognicode_core::interface::mcp::cursor::CursorError;
    let err = CursorError {
        kind: CursorErrorKind::WorkspaceMismatch,
        reason: "cursor workspace='/repo/a' but request workspace='/repo/b'".to_string(),
    };
    let v = serde_json::to_value(&err).expect("serialize");
    assert_eq!(
        v.get("kind").and_then(Value::as_str),
        Some("workspace_mismatch"),
        "the wire `kind` is a stable snake_case string"
    );
    assert_eq!(
        v.get("reason").and_then(Value::as_str),
        Some("cursor workspace='/repo/a' but request workspace='/repo/b'"),
        "the reason preserves the cause verbatim"
    );
}
