//! Stable JSON rendering of evidence rows, shared by the CLI and the MCP tools.

/// E1.W3 — Stable JSON schema for `cognicode evidence` output AND the
/// `list_evidence` / `search_evidence` MCP tools.
///
/// Same field names and types on both surfaces, so the equivalence
/// test in `cognicode-cli/tests/evidence_cli_mcp_equivalence.rs`
/// can byte-compare the two outputs (CLI JSON vs MCP tool result).
///
/// Made `pub(crate)` so the MCP adapter can call it without leaking
/// it on the public surface. The shape is the contract — adding a
/// field here MUST also be reflected on the MCP tool definition
/// (and vice-versa) to keep the equivalence test green.
pub fn render_evidence_rows_json(
    rows: Vec<crate::domain::ports::evidence_store::EvidenceSummary>,
) -> String {
    use crate::domain::ports::evidence_store::EvidenceKind;

    let payload: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "kind": match r.kind {
                    EvidenceKind::Log => "log",
                    EvidenceKind::Trace => "trace",
                    EvidenceKind::Measurement => "measurement",
                    EvidenceKind::External => "external",
                },
                "source_path": r.source_path,
                "excerpt": r.excerpt,
                "confidence": r.confidence,
            })
        })
        .collect();
    // Pretty for human readability, but stable. `serde_json::to_string_pretty`
    // sorts object keys alphabetically, which is the contract we
    // pin in the equivalence test.
    serde_json::to_string_pretty(&payload)
        .unwrap_or_else(|e| format!("failed to serialize evidence rows: {e}"))
}
