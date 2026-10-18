//! Behaviour of the two tools this unit wired into `tools/call`.
//!
//! MEDIDO 2026-10-18. `tools/list` announced four tools that `tools/call`
//! answered with `InterfaceError::ToolNotFound`. Two of them had a real
//! capability behind them and were pure wiring (`composite_impact_query`,
//! `safe_refactor_preview`); the other two had none and were removed from
//! the catalogue. The invariant
//! `mcp_roundtrip_tests::tool_surface_parity::every_advertised_tool_has_a
//! _dispatch_arm` proves the arms exist — these prove the arms *do
//! something*.
//!
//! Both handlers are read-only, which is the property worth pinning: the
//! whole point of exposing them was that an agent can investigate a
//! refactor or an impact without a write capability anywhere in the call.

use std::io::Write;
use std::path::PathBuf;

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::interface::mcp::handlers::{
    HandlerContext, HandlerError, handle_composite_impact_query,
    refactor_handlers::handle_safe_refactor_preview,
};
use cognicode_core::interface::mcp::schemas::{
    CompositeImpactQueryInput, SafeRefactorPreviewInput,
};
use cognicode_core::refactor::plan::{PlannedEdit, build_plan};

fn ctx(root: &std::path::Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(root.to_path_buf())
        .build()
}

fn write_file(path: &std::path::Path, body: &[u8]) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(body).unwrap();
    f.sync_all().unwrap();
}

/// A plan that replaces `world` with `Rust` in `a.txt`.
fn plan_fixture(root: &std::path::Path) -> PathBuf {
    let abs = root.join("a.txt");
    write_file(&abs, b"hello world\n");
    let plan = build_plan(
        root.to_path_buf(),
        AnalysisBasis::for_workspace(root.to_path_buf()),
        vec![abs.clone()],
        vec![PlannedEdit {
            file: abs,
            byte_range: 6..11,
            replacement: "Rust".to_string(),
            deterministic: true,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("build_plan");
    let plan_path = root.join("plan.json");
    write_file(
        &plan_path,
        serde_json::to_vec(&plan)
            .expect("serialise plan")
            .as_slice(),
    );
    plan_path
}

// ----------------------------------------------------------------------------
// composite_impact_query
// ----------------------------------------------------------------------------

/// The load-bearing B4.W5 invariant travels over MCP too: whatever the
/// caller asks about, the answer is evidence and never a verdict.
#[tokio::test]
async fn composite_impact_query_refuses_the_verdict_over_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();

    let out = handle_composite_impact_query(
        &ctx(&root),
        CompositeImpactQueryInput {
            qualified_name: "alpha::beta".to_string(),
            file_path: Some(root.join("a.txt").display().to_string()),
        },
    )
    .await
    .expect("a read-only composite query must answer");

    assert!(
        out.verdict_refused,
        "the composite result must always refuse the verdict; over MCP a \
         client reads this field and nothing else"
    );
    assert_eq!(
        out.verdict_refused_reason,
        cognicode_core::impact::composite::COMPOSITE_VERDICT_REFUSED_REASON,
        "the reason is a pinned constant, not a per-call string"
    );
}

/// A missing `file_path` must not be filled with a fabricated directory.
///
/// Pinned at the conversion, not through the handler. The only consumer of
/// that field is the `file_path` argument on the follow-up tools the
/// composite result suggests, and for an arbitrary symbol that list is
/// empty — so asserting through the handler here would have been a test
/// that cannot fail, which is the shape of the defect this unit exists to
/// remove.
#[test]
fn composite_impact_query_does_not_invent_a_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();

    let query = CompositeImpactQueryInput {
        qualified_name: "alpha::beta".to_string(),
        file_path: None,
    }
    .into_core_query(&root);

    assert_eq!(
        query.symbol.file_path,
        PathBuf::new(),
        "an absent file_path must stay absent, not become {}/<unspecified>",
        root.display()
    );
    assert_eq!(query.workspace_root, root);

    // The other direction still carries what the caller sent.
    let given = CompositeImpactQueryInput {
        qualified_name: "alpha::beta".to_string(),
        file_path: Some(root.join("a.txt").display().to_string()),
    }
    .into_core_query(&root);
    assert_eq!(given.symbol.file_path, root.join("a.txt"));
}

// ----------------------------------------------------------------------------
// safe_refactor_preview
// ----------------------------------------------------------------------------

#[tokio::test]
async fn safe_refactor_preview_renders_a_plan_from_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan_path = plan_fixture(&root);
    let before = std::fs::read(root.join("a.txt")).unwrap();

    let out = handle_safe_refactor_preview(
        &ctx(&root),
        SafeRefactorPreviewInput {
            plan_path: Some(plan_path.display().to_string()),
            plan_json: None,
        },
    )
    .await
    .expect("a read-only preview must answer");

    assert_eq!(out.file_diffs.len(), 1, "the plan edits one file");
    assert!(
        !out.workspace_state_digest.is_empty(),
        "the digest is what a future authorized apply compares; an empty \
         one would make that comparison vacuous"
    );
    assert_eq!(
        std::fs::read(root.join("a.txt")).unwrap(),
        before,
        "the preview rendered but the file changed — this tool is read-only"
    );
}

/// `oneOf` in the catalogue says exactly one of the two. Enforcing it here
/// is what makes the two-arm input an ADT in practice: both, and neither,
/// are caller errors with a message that names the mistake.
#[tokio::test]
async fn safe_refactor_preview_rejects_both_and_neither() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan_path = plan_fixture(&root);

    let both = handle_safe_refactor_preview(
        &ctx(&root),
        SafeRefactorPreviewInput {
            plan_path: Some(plan_path.display().to_string()),
            plan_json: Some("{}".to_string()),
        },
    )
    .await;
    let neither = handle_safe_refactor_preview(
        &ctx(&root),
        SafeRefactorPreviewInput {
            plan_path: None,
            plan_json: None,
        },
    )
    .await;

    assert!(
        both.is_err() && neither.is_err(),
        "the input is `oneOf` in the catalogue; both arms at once and no arm \
         at all are both caller errors (both={both:?}, neither={neither:?})"
    );
}

/// The read path is confined to the workspace like every other tool.
#[tokio::test]
async fn safe_refactor_preview_confines_the_plan_path_to_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let plan_path = plan_fixture(outside.path());

    let out = handle_safe_refactor_preview(
        &ctx(&root),
        SafeRefactorPreviewInput {
            plan_path: Some(plan_path.display().to_string()),
            plan_json: None,
        },
    )
    .await;

    // Assert the *kind* of error, not merely that there was one.
    //
    // Both layers refuse, on purpose: `render_preview` owns the
    // workspace-escape guard on the plan's own target files, and this
    // handler's `validate_path` rejects the plan file before it is even
    // read. A plain `is_err()` cannot tell them apart — removing
    // `validate_path` leaves it green, with the core guard doing the work.
    // That is a self-satisfying assertion, so this names the layer that
    // must fire: the cheaper one, at the boundary, before any read.
    assert!(
        matches!(out, Err(HandlerError::Security(_))),
        "a plan outside the working directory must be refused by the path \
         validator, not fail incidentally later: {out:?}"
    );
}

/// The plan file being wrong is the caller's problem, and the error says so.
///
/// Pinning the *kind*, not just "it failed": a handler that mapped every
/// plan-file error to `Internal` would still pass a plain `is_err()` while
/// telling the operator their server broke because of a typo. `NotFound`
/// versus `InvalidInput` is also the difference between "wrong path" and
/// "wrong content", which is the first thing an agent needs to know.
#[tokio::test]
async fn safe_refactor_preview_blames_the_plan_not_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let ctx = ctx(&root);

    let missing = handle_safe_refactor_preview(
        &ctx,
        SafeRefactorPreviewInput {
            plan_path: Some(root.join("nope.json").display().to_string()),
            plan_json: None,
        },
    )
    .await;
    assert!(
        matches!(missing, Err(HandlerError::NotFound(_))),
        "a plan file that is not there is NotFound, not a server fault: {missing:?}"
    );

    write_file(&root.join("garbage.json"), b"{ not json");
    let garbage = handle_safe_refactor_preview(
        &ctx,
        SafeRefactorPreviewInput {
            plan_path: Some(root.join("garbage.json").display().to_string()),
            plan_json: None,
        },
    )
    .await;
    assert!(
        matches!(garbage, Err(HandlerError::InvalidInput(_))),
        "a plan file that does not parse is InvalidInput, not a server \
         fault: {garbage:?}"
    );

    let inline = handle_safe_refactor_preview(
        &ctx,
        SafeRefactorPreviewInput {
            plan_path: None,
            plan_json: Some("{ not json".to_string()),
        },
    )
    .await;
    assert!(
        matches!(inline, Err(HandlerError::InvalidInput(_))),
        "the inline arm fails the same way the file arm does: {inline:?}"
    );
}
