//! RED characterization for `safe_refactor` rename: the advertised contract
//! and the handler disagree about what `target` means.
//!
//! `product/tools.json` advertises `target` as "Target symbol name". The
//! handler resolves it as a *path*: `working_dir.join(&input.target)`, and
//! `RenameSymbolCommand` is built with `file_path_str` = that join. Measured
//! against the real release server over stdio JSON-RPC:
//!
//!   target="old_name"  -> "Failed to read <cwd>/old_name: No such file or
//!                         directory"
//!   + explicit file_path=.../old_name (an existing file)
//!                     -> "Unsupported file type: <cwd>/old_name"
//!
//! So a real symbol name can never succeed: without a file named after the
//! symbol it is "No such file", and with one it is "Unsupported file type".
//! This is an advertised-but-nonfunctional surface, not a bad argument.
//!
//! The pre-existing unit test missed it because it asserts only
//! `result.is_ok()`, which is true even when the payload says
//! `success: false` with an error message. That is why this file checks the
//! payload, not just the Result.

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::{RefactorAction, SafeRefactorInput};

/// `success` is the field a consumer reads. `is_ok()` on the Result is not
/// enough: a handler may return Ok with `success: false` and an error string,
/// which is what the binary does today.
fn rename_success(output: &cognicode_core::interface::mcp::schemas::SafeRefactorOutput) -> bool {
    output.success && output.error_message.as_deref().unwrap_or("").is_empty()
}

#[tokio::test]
async fn rename_accepts_a_symbol_name_as_advertised_not_a_filename() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path();

    // A real Rust file containing a real symbol, plus a call site so the
    // rename has something to resolve against.
    let rust_file = dir.join("lib.rs");
    std::fs::write(
        &rust_file,
        "pub fn old_name(a: i32) -> i32 {\n    a + 1\n}\n\n\
         pub fn caller() -> i32 {\n    old_name(1)\n}\n",
    )
    .unwrap();

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.to_path_buf())
        .build();

    // Exactly what the advertised schema accepts: an action, a target symbol
    // name, and action-specific params. No filename anywhere, because the
    // schema never asks for one.
    let input = SafeRefactorInput {
        action: RefactorAction::Rename,
        target: "old_name".to_string(),
        params: Some(serde_json::json!({ "new_name": "new_name" })),
    };

    match handle(&ctx, input).await {
        Outcome::Renamed(changes) => assert!(
            !changes.is_empty(),
            "a successful rename must report the change; an empty changes \
             list means nothing was actually renamed"
        ),
        Outcome::Refused(msg) => panic!(
            "rename of a real symbol must succeed per the advertised schema \
             (target = symbol name), but it was refused: {msg}"
        ),
    }
}

#[tokio::test]
async fn rename_reports_a_real_error_when_it_cannot_run() {
    // The counterpart guard: whatever we do to fix the path/symbol confusion,
    // a failure must stay a failure. If this ever starts returning
    // success=true with no changes, the tool has begun faking the refactor.
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path();

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.to_path_buf())
        .build();

    // No source file at all: nothing to rename, and the tool must say so.
    let input = SafeRefactorInput {
        action: RefactorAction::Rename,
        target: "definitely_absent_symbol".to_string(),
        params: Some(serde_json::json!({ "new_name": "whatever" })),
    };

    match handle(&ctx, input).await {
        Outcome::Refused(msg) => assert!(
            !msg.trim().is_empty(),
            "a refusal must carry a message explaining why"
        ),
        Outcome::Renamed(changes) => panic!(
            "renaming a symbol that no file defines must not report success; \
             got {len} fabricated change(s)",
            len = changes.len()
        ),
    }
}

/// A refusal may arrive two ways, and both are correct behaviour:
///
///   * `Err(InvalidInput(..))` — the handler refused up front, e.g. "No file
///     defines a symbol named ...". Nothing was touched.
///   * `Ok` with `success: false` and an `error_message` — the payload form,
///     which is what the binary returned before the fix.
///
/// `Result::is_ok()` alone cannot tell success from refusal, which is exactly
/// why the pre-existing unit test passed while the tool was broken. So this
/// helper folds both shapes into one verdict.
#[derive(Debug)]
enum Outcome {
    Renamed(Vec<cognicode_core::interface::mcp::schemas::ChangeEntry>),
    Refused(String),
}

async fn handle(ctx: &HandlerContext, input: SafeRefactorInput) -> Outcome {
    use cognicode_core::interface::mcp::handlers::refactor_handlers::handle_safe_refactor;
    match handle_safe_refactor(ctx, input).await {
        Ok(output) => {
            if rename_success(&output) {
                Outcome::Renamed(output.changes)
            } else {
                Outcome::Refused(
                    output
                        .error_message
                        .unwrap_or_else(|| "refused with no message".to_string()),
                )
            }
        }
        Err(e) => Outcome::Refused(format!("{e}")),
    }
}
