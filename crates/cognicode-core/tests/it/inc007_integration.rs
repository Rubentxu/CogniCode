//! INC-007 R4.5: Verify that `score_scenario` returns a numeric `correctitud`
//! (>= G4_THRESHOLD = 90) for a `read_file` response reconstructed by H4.3,
//! when the manifest uses `ground_truth.code.contains` (substring presence).
//!
//! CR-00c: hermeticity. The previous version of this test loaded the actual
//! `reconstructed.json` from a sandbox run (path: `sandbox/results-runs/
//! <timestamp>/<scenario>/<timestamp>/reconstructed.json`). That artefact is
//! NOT versioned, so a clean clone cannot run this test. We replace the load
//! with an in-test synthesis that preserves the semantic surface the test
//! cares about:
//!   - content >= 21 KB (the original test asserted a 21 KB+ reconstruction);
//!   - the content contains the substring the ground-truth searches for
//!     (`//! [![github]]`).
//!     The exact bytes do not matter for the scorer's behaviour; only the
//!     length threshold and the substring match do. This keeps the test
//!     reproducible from Git alone.

use cognicode_core::sandbox_core::ground_truth::GroundTruth;
use cognicode_core::sandbox_core::scoring::{ExecutionMetadata, score_scenario};

/// The substring the ground-truth will search for. Must appear inside
/// the synthetic reconstructed content below.
const SUBSTRING: &str = "//! [![github]]";

/// Minimum reconstructed content length the test requires. The original
/// reconstructed `anyhow/src/lib.rs` was 21 KB; we use 22 KB to keep
/// a small margin above the threshold.
const MIN_CONTENT_LEN: usize = 22_000;

fn synthetic_reconstructed_content() -> String {
    // Header: contains the substring the ground-truth searches for.
    // Body: padding to push length past MIN_CONTENT_LEN without including
    // SUBSTRING again (we want exactly one match, mirroring the
    // real reconstructed anyhow/src/lib.rs which had one docstring
    // banner and one body of declarations).
    let header = "//! [![github]]\n//! crate-level documentation for the reconstructed fixture.\n\
         //! this header is intentional: it is the substring the ground-truth looks for.\n\n"
        .to_string();
    let body_needed = MIN_CONTENT_LEN.saturating_sub(header.len());
    // 'a'..'z' repeated. Note: no newline-collapse concerns; the substring
    // matcher operates on raw bytes.
    let mut body = String::with_capacity(body_needed);
    let alphabet = b"abcdefghijklmnopqrstuvwxyz\n";
    while body.len() < body_needed {
        body.push_str(std::str::from_utf8(alphabet).unwrap());
    }
    body.truncate(body_needed);
    let mut full = String::with_capacity(header.len() + body.len());
    full.push_str(&header);
    full.push_str(&body);
    assert!(
        full.len() >= MIN_CONTENT_LEN,
        "synthetic content must be >= {MIN_CONTENT_LEN} bytes, got {}",
        full.len()
    );
    assert!(
        full.contains(SUBSTRING),
        "synthetic content must contain the substring '{SUBSTRING}'"
    );
    full
}

#[test]
fn integration_score_anyhow_contains_with_reconstructed_response() {
    let full_content = synthetic_reconstructed_content();

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
