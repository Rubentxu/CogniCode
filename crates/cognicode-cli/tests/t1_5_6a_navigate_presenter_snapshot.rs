//! T1-5.6a — Snapshot tests for the `NavigateResult` text + JSON dispatch.
//!
//! Estado pre-T1-5.6a:
//!   - `execute_navigate` emitted ~25 inline `println!`/`eprintln!` calls
//!     across its three arms (definition / hover / references).
//!   - The library did the formatting, so the bin (cognicode-cli) could
//!     not switch to `--format json` for `navigate` even though every
//!     other migrated sub-command gained that dispatch in T1-5.5.
//!   - The text shape was not pinned anywhere; a regression in a label,
//!     a number format, or the column alignment would only surface when
//!     a user read the CLI output and complained.
//!
//! What this RED protects:
//!   - The exact text emitted by each `NavigateResult` variant is
//!     pinned below. Any change to wording, capitalization, the number
//!     of spaces in the "File: / Line: / Column:" block, the "(workspace: ...)"
//!     suffix on the progress line, or the `[KIND]` Debug format of
//!     `ReferenceKind` breaks the corresponding test and forces the
//!     author to consciously update the snapshot.
//!   - The JSON shape of each variant: `kind` discriminator, position
//!     fields, and the inner types (`DefinitionLocation`, `HoverResult`,
//!     `ReferenceEntry`) are byte-for-byte parseable.
//!
//! Why snapshot and not golden-file:
//!   - Same reason as `t1_5_3d_index_presenter_snapshot.rs`: no insta
//!     in `Cargo.toml`; expected text is short and inline; a reviewer
//!     can verify by reading.
//!
//! Why this lives under `cognicode-cli/tests/` and not `cognicode-core/tests/`:
//!   - The presenter (formatting) moved to the bin in T1-5.5; the data
//!     side (`NavigateResult`, `NavigateError`) lives in cognicode-core.
//!     The text/JSON dispatch is a bin concern, so the test belongs in
//!     the bin's `tests/` (the same place the existing T1-5.4 graph and
//!     T1-5.5 dispatch snapshots live).

use cognicode_cli::presenter::{
    present_navigate_definition, present_navigate_hover, present_navigate_references,
    present_navigate_result_json,
};
use cognicode_core::domain::traits::code_intelligence::ReferenceKind;
use cognicode_core::interface::cli::result::{
    DefinitionLocation, HoverResult, NavigateResult, ReferenceEntry,
};
use serde_json::Value;

fn render(result: &NavigateResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    // Match the present_navigate_result dispatcher in the bin:
    // each variant has its own presenter. The snapshot pins
    // each one independently.
    match result {
        NavigateResult::Definition { .. } => {
            present_navigate_definition(result, &mut buf).expect("definition write infallible");
        }
        NavigateResult::Hover { .. } => {
            present_navigate_hover(result, &mut buf).expect("hover write infallible");
        }
        NavigateResult::References { .. } => {
            present_navigate_references(result, &mut buf).expect("references write infallible");
        }
    }
    String::from_utf8(buf).expect("presenter emits UTF-8 text")
}

fn render_json(result: &NavigateResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_navigate_result_json(result, &mut buf)
        .expect("presenter write to Vec<u8> is infallible");
    String::from_utf8(buf).expect("presenter emits UTF-8 JSON")
}

fn assert_kind(out: &str, expected: &str) {
    let parsed: Value = serde_json::from_str(out)
        .unwrap_or_else(|e| panic!("presenter output must be parseable JSON: {e}; got {out:?}"));
    let kind = parsed
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("presenter JSON must carry a 'kind' discriminator; got {out:?}"));
    assert_eq!(kind, expected, "kind mismatch in {out:?}");
}

// ===================================================================
// Definition
// ===================================================================

#[test]
fn snapshot_navigate_definition_found() {
    // The pre-migration text (commands.rs:1613..1630):
    //   Go to definition: src/main.rs:42:10 (workspace: .)
    //   Connecting to LSP server...
    //   Definition found:
    //     File:   src/lib.rs
    //     Line:   100
    //     Column: 5
    //
    // The 2-space indent in front of "File: / Line: / Column:" is
    // part of the contract; the inner column widths (4 / 4 / 6) are
    // pin targets — a downstream `column -t` consumer depends on them.
    let r = NavigateResult::Definition {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        location: Some(DefinitionLocation {
            file: "src/lib.rs".to_string(),
            line: 100,
            column: 5,
        }),
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Go to definition: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         Definition found:\n\
         \x20\x20File:   src/lib.rs\n\
         \x20\x20Line:   100\n\
         \x20\x20Column: 5\n",
        "Definition found text shape changed; check progress line + result block + indent"
    );
}

#[test]
fn snapshot_navigate_definition_not_found() {
    // The pre-migration text (commands.rs:1632):
    //   Go to definition: src/main.rs:42:10 (workspace: .)
    //   Connecting to LSP server...
    //   No definition found for src/main.rs:42
    //
    // The "no result" branch keeps the progress lines and emits a
    // single line using the original `file` + `line` (not the
    // 1-indexed line from the Location).
    let r = NavigateResult::Definition {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        location: None,
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Go to definition: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         No definition found for src/main.rs:42\n",
        "Definition not-found text changed; check the 'No definition found for' wording"
    );
}

// ===================================================================
// Hover
// ===================================================================

#[test]
fn snapshot_navigate_hover_with_docs() {
    // The pre-migration text (commands.rs:1642..1658):
    //   Hover info: src/main.rs:42:10 (workspace: .)
    //   Connecting to LSP server...
    //   Hover information:
    //     Type:    fn() -> u32
    //     Docs:    Returns the answer.
    //
    // The 2-space indent + "Type:    " (4 spaces after colon) and
    // "Docs:    " (4 spaces) are part of the contract. The
    // documentation block is OPTIONAL (`info.documentation` is
    // `Option<String>`); this test exercises the populated branch;
    // `snapshot_navigate_hover_without_docs` exercises the absent.
    let r = NavigateResult::Hover {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        info: Some(HoverResult {
            type_info: "fn() -> u32".to_string(),
            documentation: Some("Returns the answer.".to_string()),
        }),
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Hover info: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         Hover information:\n\
         \x20\x20Type:    fn() -> u32\n\
         \x20\x20Docs:    Returns the answer.\n",
        "Hover-with-docs text shape changed; check the 4-space gap after 'Type:' and 'Docs:'"
    );
}

#[test]
fn snapshot_navigate_hover_without_docs() {
    // Documentation optional: when `None`, the "Docs:" line is
    // ABSENT, not emitted with an empty body.
    let r = NavigateResult::Hover {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        info: Some(HoverResult {
            type_info: "i32".to_string(),
            documentation: None,
        }),
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Hover info: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         Hover information:\n\
         \x20\x20Type:    i32\n",
        "Hover-without-docs text changed; 'Docs:' line must be absent when documentation is None"
    );
    // Belt-and-braces: confirm no stray "Docs:" leaked.
    assert!(
        !out.contains("Docs:"),
        "docs=None must not emit the 'Docs:' line; got: {out:?}"
    );
}

#[test]
fn snapshot_navigate_hover_not_found() {
    let r = NavigateResult::Hover {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        info: None,
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Hover info: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         No hover information found for src/main.rs:42\n",
        "Hover not-found text changed; check 'No hover information found for' wording"
    );
}

// ===================================================================
// References
// ===================================================================

#[test]
fn snapshot_navigate_references_with_results() {
    // The pre-migration text (commands.rs:1675..1704):
    //   Find references: src/main.rs:42:10 (workspace: .)
    //   Connecting to LSP server...
    //   Found N reference(s):
    //     FILE:LINE:COL [KIND] in CONTAINER
    //     ...
    //
    // `line` in `ReferenceEntry` is 1-indexed (pre-migration does
    // `r.location.line() + 1`). The `[KIND]` is the Debug format of
    // `ReferenceKind` (PascalCase, e.g. "Read", "Call", "Import").
    let r = NavigateResult::References {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        include_declaration: true,
        references: vec![
            ReferenceEntry {
                file: "src/lib.rs".to_string(),
                line: 7,
                column: 5,
                kind: ReferenceKind::Read,
                container: "module alpha".to_string(),
            },
            ReferenceEntry {
                file: "src/main.rs".to_string(),
                line: 100,
                column: 12,
                kind: ReferenceKind::Call,
                container: "fn main".to_string(),
            },
        ],
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Find references: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         Found 2 reference(s):\n\
         \x20\x20src/lib.rs:7:5 [Read] in module alpha\n\
         \x20\x20src/main.rs:100:12 [Call] in fn main\n",
        "References found text changed; check the '[KIND]' Debug format and 'in CONTAINER' suffix"
    );
}

#[test]
fn snapshot_navigate_references_not_found() {
    // Empty `references` Vec produces the dedicated
    // "No references found for ..." message, NOT "Found 0 reference(s):".
    let r = NavigateResult::References {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        include_declaration: true,
        references: vec![],
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Find references: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         No references found for src/main.rs:42\n",
        "References not-found text changed; check the 'No references found for' wording"
    );
    // Belt-and-braces: the "Found 0 reference(s)" branch must NOT
    // appear even when `references.is_empty()`.
    assert!(
        !out.contains("Found 0"),
        "empty references Vec must emit 'No references found for', not 'Found 0': {out:?}"
    );
}

#[test]
fn snapshot_navigate_references_unknown_container() {
    // When the LSP returns `container: None` for a reference, the
    // pre-migration code substitutes "(unknown)". The data side
    // carries that already-substituted String (the bin doesn't know
    // about `Option<String>` here).
    let r = NavigateResult::References {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        include_declaration: false,
        references: vec![ReferenceEntry {
            file: "src/main.rs".to_string(),
            line: 50,
            column: 5,
            kind: ReferenceKind::Type,
            container: "(unknown)".to_string(),
        }],
    };
    let out = render(&r);
    assert_eq!(
        out,
        "Find references: src/main.rs:42:10 (workspace: .)\n\
         Connecting to LSP server...\n\
         Found 1 reference(s):\n\
         \x20\x20src/main.rs:50:5 [Type] in (unknown)\n",
        "References 'unknown' container changed; check the '(unknown)' substitution"
    );
}

// ===================================================================
// JSON dispatch — the bin's `--format json` path
// ===================================================================

#[test]
fn json_navigate_definition_shape() {
    let r = NavigateResult::Definition {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        location: Some(DefinitionLocation {
            file: "src/lib.rs".to_string(),
            line: 100,
            column: 5,
        }),
    };
    let out = render_json(&r);
    assert_kind(&out, "definition");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["file"], "src/main.rs");
    assert_eq!(v["line"], 42);
    assert_eq!(v["column"], 10);
    assert_eq!(v["workspace"], ".");
    assert!(
        v["location"].is_object(),
        "Definition present must carry a location object"
    );
    assert_eq!(v["location"]["file"], "src/lib.rs");
    assert_eq!(v["location"]["line"], 100);
    assert_eq!(v["location"]["column"], 5);
}

#[test]
fn json_navigate_definition_not_found_shape() {
    let r = NavigateResult::Definition {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        location: None,
    };
    let out = render_json(&r);
    assert_kind(&out, "definition");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["location"].is_null(),
        "Definition absent must serialise location as null: {v:?}"
    );
}

#[test]
fn json_navigate_hover_with_docs_shape() {
    let r = NavigateResult::Hover {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        info: Some(HoverResult {
            type_info: "fn() -> u32".to_string(),
            documentation: Some("Returns the answer.".to_string()),
        }),
    };
    let out = render_json(&r);
    assert_kind(&out, "hover");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["file"], "src/main.rs");
    assert_eq!(v["info"]["type_info"], "fn() -> u32");
    assert_eq!(v["info"]["documentation"], "Returns the answer.");
}

#[test]
fn json_navigate_hover_without_docs_shape() {
    let r = NavigateResult::Hover {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        info: Some(HoverResult {
            type_info: "i32".to_string(),
            documentation: None,
        }),
    };
    let out = render_json(&r);
    assert_kind(&out, "hover");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["info"]["documentation"].is_null(),
        "Documentation absence must serialise as null: {v:?}"
    );
}

#[test]
fn json_navigate_references_shape() {
    let r = NavigateResult::References {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        include_declaration: true,
        references: vec![ReferenceEntry {
            file: "src/lib.rs".to_string(),
            line: 7,
            column: 5,
            kind: ReferenceKind::Read,
            container: "module alpha".to_string(),
        }],
    };
    let out = render_json(&r);
    assert_kind(&out, "references");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["include_declaration"], true);
    assert_eq!(v["references"].as_array().unwrap().len(), 1);
    assert_eq!(v["references"][0]["file"], "src/lib.rs");
    assert_eq!(v["references"][0]["kind"], "Read");
    assert_eq!(v["references"][0]["container"], "module alpha");
}

#[test]
fn json_navigate_references_empty_shape() {
    let r = NavigateResult::References {
        file: "src/main.rs".to_string(),
        line: 42,
        column: 10,
        workspace: ".".to_string(),
        include_declaration: true,
        references: vec![],
    };
    let out = render_json(&r);
    assert_kind(&out, "references");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["references"].as_array().unwrap().len(), 0);
}
