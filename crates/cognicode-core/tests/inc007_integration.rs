use cognicode_core::sandbox_core::ground_truth::GroundTruth;
use cognicode_core::sandbox_core::scoring::{ExecutionMetadata, score_scenario};

/// INC-007 R4.5: Verify that `score_scenario` returns a numeric `correctitud`
/// (>= G4_THRESHOLD = 90) for a `read_file` response reconstructed by H4.3,
/// when the manifest uses `ground_truth.code.contains` (substring presence).
///
/// Loads the actual reconstructed.json produced by the latest campaign run
/// to avoid hand-rolling a 21 KB file in test fixtures.
#[test]
fn integration_score_anyhow_contains_with_reconstructed_response() {
    // CARGO_MANIFEST_DIR is the workspace root for this crate.
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let recon_path = format!(
        "{}/../../sandbox/results-runs/20260920T182612/rust_tier1_anyhow_read_source_default/20260920T182613/reconstructed.json",
        manifest_dir
    );
    let recon_text = std::fs::read_to_string(&recon_path)
        .unwrap_or_else(|e| panic!("read {}: {}", recon_path, e));
    let recon: serde_json::Value =
        serde_json::from_str(&recon_text).expect("parse reconstructed.json");
    let full_content = recon["content"]
        .as_str()
        .expect("content present")
        .to_string();
    assert!(
        full_content.len() >= 21_000,
        "expected 21 KB+ reconstructed anyhow/src/lib.rs, got {} bytes",
        full_content.len()
    );

    // Inner JSON shape the H4.3 reconstructor produces.
    let inner = serde_json::json!({
        "content": full_content,
        "total_lines": 730,
        "truncated": false,
        "has_more": false,
        "mode": "raw",
        "metadata": {"path": "/tmp/anyhow/src/lib.rs", "size": full_content.len()}
    });

    // MCP response shape that unwrap_mcp_content parses.
    let response = serde_json::json!({
        "id": 1,
        "jsonrpc": "2.0",
        "result": {
            "content": [{"text": inner.to_string(), "type": "text"}],
            "isError": false
        }
    });

    let gt_json = r#"{
        "code": {
            "file": "sandbox/repos/anyhow/src/lib.rs",
            "line": 1,
            "col": 1,
            "content": "",
            "contains": "//! [![github]]"
        }
    }"#;
    let gt: GroundTruth = serde_json::from_str(gt_json).expect("parse gt");

    let score = score_scenario(
        "read_file",
        "rust",
        "tier1_anyhow_read_source_default",
        &response,
        &Some(gt),
        &None,
        100,
        ExecutionMetadata::with_errors(0, 0, 1),
    );

    println!("correctitud: {:?}", score.correctitud);
    println!("code_match: {:#?}", score.code_match);

    assert!(
        score.correctitud.is_some(),
        "correctitud should be Some for a ground-truth with `contains` over a complete reconstruction"
    );
    let corr = score.correctitud.unwrap();
    assert!(
        corr >= 90.0,
        "correctitud must be >= G4_THRESHOLD (90), got {}",
        corr
    );
}
