//! B3.W2 — Progressive context routing for agent code understanding.
//!
//! Estado pre-B3.W2:
//!   - 6 tools (`codebase_map`, `smart_search`, `get_symbol_code`,
//!     `read_file`, `search_content`, `retrieve_and_verify`) son
//!     invocables aisladamente, pero el agente tiene que saber los
//!     seis nombres y combinarlos a mano.
//!   - No hay ruta "intent-bearing": un agente que quiere "localiza
//!     la definicion de X" tiene que reconstruir la cadena
//!     `query_symbol_index` -> `get_symbol_code` -> `read_file`.
//!
//! Lo que cierra este test:
//!   - `Intent` modela la intencion del agente (Locate, Summarize,
//!     Verify, Read) y `route_intent` devuelve los tool references
//!     que llevan a un primer resultado util sin que el agente
//!     conozca los seis nombres.
//!   - `route_intent` cubre los 6 tools: cada uno aparece al
//!     menos en una variante de `Intent`.
//!   - `Locate` siempre enruta a `query_symbol_index` Y
//!     `get_symbol_code` (encadenamiento definicion -> codigo).
//!   - `Verify` siempre enruta a `retrieve_and_verify` (cierre de
//!     cadena: el agente que afirma algo tiene que verificarlo).
//!   - `Read` siempre enruta a `read_file` (cierre de cadena:
//!     paginacion para archivos grandes).
//!   - El catalogo `product/tools.json` lleva `intent_tags` por
//!     tool, en paridad con `route_intent`: si una tool aparece en
//!     un routing, su `intent_tags` lista esa `Intent` variante.
//!   - `cognicode inspect` es un scaffold: parsea el input como
//!     best-effort, imprime "PROPOSED: contract under review; see
//!     B3.W5" a stderr, y devuelve exit 0 (no panic) en cualquier
//!     entrada.
//!
//! Politica B3.W2: el scaffold NO pina contrato final. B3.W5
//! cierra el shape cuando se valide el routing contra datos
//! reales (workspace de smoke-test). El test pina que el
//! scaffold compila, no panic, y honra el status `PROPOSED`.

use cognicode_core::interface::catalog::{load_tool_catalog, ToolDescriptor};
use cognicode_core::interface::intent::{route_intent, Intent};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

// -----------------------------------------------------------------------
// Routing function: tool presence
// -----------------------------------------------------------------------

#[test]
fn route_intent_locate_returns_query_symbol_index() {
    let route = route_intent(&Intent::Locate {
        symbol: "foo".to_string(),
    });
    let names: Vec<&str> = route.iter().map(|r| r.tool.as_str()).collect();
    assert!(
        names.contains(&"query_symbol_index"),
        "Locate must route to query_symbol_index; got {names:?}"
    );
}

#[test]
fn route_intent_locate_returns_get_symbol_code() {
    let route = route_intent(&Intent::Locate {
        symbol: "foo".to_string(),
    });
    let names: Vec<&str> = route.iter().map(|r| r.tool.as_str()).collect();
    assert!(
        names.contains(&"get_symbol_code"),
        "Locate must route to get_symbol_code; got {names:?}"
    );
}

#[test]
fn route_intent_summarize_returns_codebase_map() {
    let route = route_intent(&Intent::Summarize {
        scope: "crates/cognicode-core".to_string(),
    });
    let names: Vec<&str> = route.iter().map(|r| r.tool.as_str()).collect();
    assert!(
        names.contains(&"codebase_map"),
        "Summarize must route to codebase_map; got {names:?}"
    );
}

#[test]
fn route_intent_verify_returns_retrieve_and_verify() {
    let route = route_intent(&Intent::Verify {
        claim: "the answer is 42".to_string(),
    });
    let names: Vec<&str> = route.iter().map(|r| r.tool.as_str()).collect();
    assert!(
        names.contains(&"retrieve_and_verify"),
        "Verify must route to retrieve_and_verify; got {names:?}"
    );
}

#[test]
fn route_intent_read_returns_read_file() {
    let route = route_intent(&Intent::Read {
        path: "crates/cognicode-core/src/lib.rs".to_string(),
        range: None,
    });
    let names: Vec<&str> = route.iter().map(|r| r.tool.as_str()).collect();
    assert!(
        names.contains(&"read_file"),
        "Read must route to read_file; got {names:?}"
    );
}

#[test]
fn route_intent_returns_non_empty_for_every_variant() {
    // Property: every Intent variant produces >= 1 ToolRef. A
    // route that returns an empty Vec would leave the agent with
    // nothing to call; that is a contract failure (B3.W2 §Riesgos).
    let cases = vec![
        Intent::Locate {
            symbol: "X".into(),
        },
        Intent::Summarize {
            scope: "src".into(),
        },
        Intent::Verify {
            claim: "claim".into(),
        },
        Intent::Read {
            path: "x.rs".into(),
            range: None,
        },
    ];
    for intent in cases {
        let route = route_intent(&intent);
        assert!(
            !route.is_empty(),
            "route_intent({intent:?}) returned empty Vec"
        );
    }
}

// -----------------------------------------------------------------------
// Coverage: the 6 tools each appear in >= 1 routing branch
// -----------------------------------------------------------------------

#[test]
fn all_six_tools_appear_in_at_least_one_route_intent_branch() {
    // The 6 B3.W2 composable tools. If one disappears from every
    // route_intent branch, it means the scaffold has drifted from
    // the WU scope. Adding a 7th tool to this list is a contract
    // change and requires a corresponding route_intent branch.
    const TARGET_TOOLS: &[&str] = &[
        "codebase_map",
        "smart_search",
        "get_symbol_code",
        "read_file",
        "search_content",
        "retrieve_and_verify",
    ];

    let intents = vec![
        Intent::Locate {
            symbol: "x".into(),
        },
        Intent::Summarize {
            scope: ".".into(),
        },
        Intent::Verify {
            claim: "c".into(),
        },
        Intent::Read {
            path: "f.rs".into(),
            range: None,
        },
    ];

    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for intent in &intents {
        for r in route_intent(intent) {
            seen.insert(Box::leak(r.tool.into_boxed_str()));
        }
    }

    for tool in TARGET_TOOLS {
        assert!(
            seen.contains(tool),
            "{tool} must appear in at least one route_intent branch; \
             seen={seen:?}"
        );
    }
}

// -----------------------------------------------------------------------
// Catalog parity: tools.json reflects route_intent via intent_tags
// -----------------------------------------------------------------------

#[test]
fn intent_tags_in_catalog_match_route_intent() {
    // Parity contract (B3.W2):
    //   For every (intent_variant, tool) pair that route_intent
    //   actually emits, the catalog entry for `tool` carries that
    //   intent variant in its `intent_tags`.
    //
    // We don't assert the converse (catalog mentions -> route
    //   returns): the catalog can advertise affordances that the
    //   current scaffold doesn't wire yet. What we DO assert is
    //   that no tool that route_intent routes to is silently
    //   unlabeled in the catalog.

    let catalog = load_tool_catalog();
    assert!(
        !catalog.is_empty(),
        "load_tool_catalog() returned empty list; \
         catalog file must be present for this test to mean anything"
    );

    let by_name: std::collections::HashMap<&str, &ToolDescriptor> = catalog
        .iter()
        .map(|d| (d.name.as_str(), d))
        .collect();

    // Map every routing into ("intent variant label", "tool name").
    let intents_and_labels: &[(&str, Intent)] = &[
        ("Locate", Intent::Locate { symbol: "s".into() }),
        (
            "Summarize",
            Intent::Summarize {
                scope: ".".into(),
            },
        ),
        ("Verify", Intent::Verify { claim: "c".into() }),
        (
            "Read",
            Intent::Read {
                path: "p.rs".into(),
                range: None,
            },
        ),
    ];

    let mut violations: Vec<String> = Vec::new();
    for (label, intent) in intents_and_labels {
        for r in route_intent(intent) {
            let descriptor = match by_name.get(r.tool.as_str()) {
                Some(d) => d,
                None => {
                    violations.push(format!(
                        "route_intent({label}) -> {} but no catalog entry exists",
                        r.tool
                    ));
                    continue;
                }
            };
            let tags_present = descriptor
                .intent_tags
                .iter()
                .any(|t| t.as_str() == *label);
            if !tags_present {
                violations.push(format!(
                    "route_intent({label}) -> {} but catalog entry has \
                     intent_tags={:?}",
                    r.tool, descriptor.intent_tags
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "intent_tags parity violations:\n{}",
        violations.join("\n")
    );
}

// -----------------------------------------------------------------------
// CLI scaffold: cognicode inspect must not panic on arbitrary input
// -----------------------------------------------------------------------

fn cognicode_bin() -> Option<PathBuf> {
    // Mirror the resolution strategy used in cognicode-cli/tests:
    // 1. CARGO_BIN_EXE_cognicode at compile time (set when running
    //    `cargo test` against the cognicode-cli crate).
    // 2. Same env var at runtime (cargo-nextest, manual wrappers).
    // 3. CARGO_TARGET_DIR override (this environment uses one).
    // 4. Workspace-relative target/.

    if let Some(p) = option_env!("CARGO_BIN_EXE_cognicode") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(p) = std::env::var_os("CARGO_BIN_EXE_cognicode") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let release = PathBuf::from(&target).join("release").join("cognicode");
        if release.exists() {
            return Some(release);
        }
        let debug = PathBuf::from(&target).join("debug").join("cognicode");
        if debug.exists() {
            return Some(debug);
        }
    }
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_default();
    let workspace_root = manifest_dir.join("..").join("..");
    let release = workspace_root.join("target").join("release").join("cognicode");
    if release.exists() {
        return Some(release.canonicalize().ok().unwrap_or(release));
    }
    let debug = workspace_root.join("target").join("debug").join("cognicode");
    if debug.exists() {
        return Some(debug.canonicalize().ok().unwrap_or(debug));
    }
    None
}

#[test]
fn inspect_subcommand_does_not_panic_on_arbitrary_input() {
    // The scaffold must compile and exit 0 on any of these shapes.
    // We don't pin the exact routing (that's B3.W5); we pin that
    // the binary survives arbitrary free-text input.
    let bin = match cognicode_bin() {
        Some(p) => p,
        None => {
            eprintln!(
                "cognicode binary not found; skipping the subprocess test. \
                 Build with `cargo build -p cognicode-cli --bin cognicode` \
                 and re-run."
            );
            return;
        }
    };

    let inputs: &[&[&str]] = &[
        &["inspect", "fn", "foo"],
        &["inspect", "path/to/file.rs:10-20"],
        &["inspect", ""],
        &["inspect", "completely", "arbitrary", "text"],
        &["inspect", "sym::Method"],
        &["inspect", "verify that 2+3=5"],
        &["inspect", "summary of crates/cognicode-core"],
    ];

    for args in inputs {
        let out = Command::new(&bin)
            .args(*args)
            .output()
            .expect("spawn cognicode binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let code = out.status.code().unwrap_or(-1);
        assert_eq!(
            code, 0,
            "inspect must exit 0 (no panic) for args={args:?}; \
             stderr={stderr}"
        );
        // The scaffold must announce its provisional status so the
        // agent does not treat the result as the production contract.
        assert!(
            stderr.contains("PROPOSED"),
            "stderr must contain the PROPOSED status marker for \
             args={args:?}; got stderr={stderr}"
        );
    }
}

// -----------------------------------------------------------------------
// Routing args shape: each ToolRef carries JSON args the caller can pass
// -----------------------------------------------------------------------

#[test]
fn route_intent_locate_args_include_symbol_name() {
    // Property: the first ToolRef of Locate must carry the symbol
    // in its args. If the scaffold produces an empty args object,
    // the caller cannot actually invoke the tool.
    let route = route_intent(&Intent::Locate {
        symbol: "MyType".to_string(),
    });
    let query_idx = route
        .iter()
        .find(|r| r.tool == "query_symbol_index")
        .expect("Locate -> query_symbol_index");
    let args: Value = query_idx.args.clone();
    let symbol_name = args.get("symbol_name").and_then(|v| v.as_str());
    assert_eq!(
        symbol_name,
        Some("MyType"),
        "query_symbol_index args must carry symbol_name=MyType; got {args}"
    );
}

#[test]
fn route_intent_read_args_include_path() {
    let route = route_intent(&Intent::Read {
        path: "src/lib.rs".to_string(),
        range: Some(std::ops::Range {
            start: 10,
            end: 20,
        }),
    });
    let reader = route
        .iter()
        .find(|r| r.tool == "read_file")
        .expect("Read -> read_file");
    let args: Value = reader.args.clone();
    let path = args.get("path").and_then(|v| v.as_str());
    assert_eq!(
        path,
        Some("src/lib.rs"),
        "read_file args must carry path=src/lib.rs; got {args}"
    );
}

// -----------------------------------------------------------------------
// Convenience: a small smoke check for the json! macro availability
// -----------------------------------------------------------------------

#[test]
fn json_macro_smoke() {
    let v = json!({ "kind": "locate", "symbol": "foo" });
    assert_eq!(v["kind"], "locate");
    assert_eq!(v["symbol"], "foo");
}