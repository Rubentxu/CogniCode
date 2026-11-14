//! H4.3 — no invented local-homonym edges (H-R4-3 regression).
//!
//! The H4 spec says: "Eliminar el atajo de `name_to_symbol` que
//! sobreescribe homónimos locales; llamadas ambiguas quedan
//! observadas sin una dependencia falsa."
//!
//! `name_to_symbol` in `per_file_graph.rs::build_file_graph` is
//! a `HashMap<String, SymbolId>` keyed by `symbol.name().to_lowercase()`.
//! When a single file declares two symbols under the same key, the
//! second insert overwrites the first. A call site that should be
//! observed as "ambiguous, no edge" is then resolved to whichever symbol
//! happened to be parsed last. That is a false dependency in the call
//! graph: an analyst chasing an impact path lands in the wrong callee.
//!
//! The fix replaces the single-symbol map with a multimap; an entry of
//! length > 1 is ambiguous and the local resolver drops the call. A
//! same-file homonym call is left as an honest observation: no edge, no
//! false dependency, and a future diagnostics pass can list it.
//!
//! ## What a homonym is
//!
//! A homonym is two declarations of the SAME name, and the key of the
//! multimap is the name the language reads. The fixture below declares
//! `Foo` twice: that is a homonym in every language, and the call is
//! ambiguous.
//!
//! A pair whose names differ only in the case —`Foo` and `foo`— is
//! NOT a homonym in a language that tells them apart, and Python is one
//! of them. The fold that used to make them one key was the index's
//! property; it is the language's now, and a language that does not
//! fold resolves the call to the declaration it names. The last test in
//! this file pins that side of the contract: the edge is published, and
//! it names the declaration the call names.
//!
//! Two tests pin the ambiguous side. The "false dependency is gone"
//! test is the bug the cleanup was designed to prevent. The
//! "no homonym = normal resolution" test is the legitimate case
//! the resolver MUST still handle: a file with a single `Foo`
//! method and a caller that invokes it must still produce a
//! call edge. Together the two assertions bound the contract
//! from both sides.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;

const FIXTURE_HOMONYMS: &str = r#"
def caller():
    Foo()

def Foo():
    pass

def Foo():
    pass
"#;

const FIXTURE_CASE_DIFFERENT: &str = r#"
def caller():
    Foo()

def Foo():
    pass

def foo():
    pass
"#;

const FIXTURE_NO_HOMONYM: &str = r#"
def caller():
    Foo()

def Foo():
    pass
"#;

/// The failure mode: a file with two homonyms of the same name
/// (`Foo` and `Foo`) and a caller that invokes `Foo`. The
/// previous code's `name_to_symbol.insert(name.to_lowercase(), id)`
/// overwrites the first entry with the second, so the call site
/// resolves to whichever symbol the parser inserted last and the call
/// graph records a false dependency. The corrected resolver
/// must drop the call: the homonym pair is genuinely ambiguous
/// under the local symbol table, and inventing an edge is the
/// failure mode the H4 spec forbids.
#[test]
fn h43_local_homonym_drops_the_call_edge() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let file_path = tmp.path().join("homonyms.py");
    std::fs::write(&file_path, FIXTURE_HOMONYMS).expect("write fixture");

    let cache = PerFileGraphCache::with_project_dir(tmp.path().to_string_lossy().into_owned());
    let report = cache.merge_with_report(&[file_path.as_path()]);

    let graph: &CallGraph = &report.graph;

    // Locate the caller symbol. Its FQN is the file path plus
    // the function name; the parser assigns it on a per-file
    // basis. `find_by_name` is case-insensitive; we additionally
    // assert the exact name so the lookup is unambiguous.
    let caller_id = graph
        .symbol_ids()
        .find(|(_, s)| s.name() == "caller")
        .map(|(id, _)| id.clone())
        .expect("fixture must declare exactly one `caller`");

    // The call site in `caller` invokes `Foo`. Under the
    // corrected local resolver the edge is dropped (ambiguous
    // homonym). Under the previous code the edge went to
    // whichever symbol was inserted last, which is the failure
    // mode. Either way, the count of call edges from `caller`
    // must be zero.
    let outgoing = graph
        .dependencies(&caller_id)
        .map(|(_from, to)| *to)
        .collect::<Vec<_>>();
    assert!(
        outgoing.is_empty(),
        "local homonym (Foo vs Foo) must not produce a call edge; \
         found {} outgoing edges from `caller`; the previous code \
         emitted a false dependency to whichever symbol the parser \
         inserted last into name_to_symbol",
        outgoing.len()
    );

    // Belt-and-braces: the global edge count is unchanged. The
    // file has three real symbols (caller, Foo, Foo) and zero
    // call edges — not one edge to the wrong callee.
    assert_eq!(
        graph.edge_count(),
        0,
        "homonym file must produce a graph with zero call edges; \
         the previous code produced one false dependency; \
         edge_count={}",
        graph.edge_count()
    );
}

/// Two declarations whose names differ only in the case are two names.
///
/// The call names `Foo`, and `Foo` is the only declaration spelled that
/// way, so the edge is published and it names that declaration. A
/// resolver that folded the case would call the pair homonyms and
/// publish a hole where an edge was — and in a language like Rust,
/// where `Config` and `config()` are the ordinary way of writing a type
/// and its constructor, that hole is the norm rather than the
/// exception.
#[test]
fn h43_names_that_differ_only_in_case_are_not_homonyms() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let file_path = tmp.path().join("cased.py");
    std::fs::write(&file_path, FIXTURE_CASE_DIFFERENT).expect("write fixture");

    let cache = PerFileGraphCache::with_project_dir(tmp.path().to_string_lossy().into_owned());
    let report = cache.merge_with_report(&[file_path.as_path()]);

    let graph: &CallGraph = &report.graph;

    let foo_id = graph
        .symbol_ids()
        .find(|(_, s)| s.name() == "Foo")
        .map(|(id, _)| id.clone())
        .expect("fixture must declare exactly one `Foo`");
    let caller_id = graph
        .symbol_ids()
        .find(|(_, s)| s.name() == "caller")
        .map(|(id, _)| id.clone())
        .expect("fixture must declare exactly one `caller`");

    assert!(
        graph
            .dependencies(&caller_id)
            .any(|(t, _kind)| t == &foo_id),
        "the call names `Foo` and `Foo` is the only declaration spelled \
         that way: the edge is not ambiguous"
    );
}

/// The legitimate case the resolver MUST still handle: a
/// file with one `Foo` method (no homonym) and a caller that
/// invokes it. The single-symbol map would have produced the
/// call edge, and the corrected multimap resolver must produce
/// it too. Without this assertion, a future fix that drops
/// every local call (or, equivalently, never matches a single
/// homonym candidate) would pass the first test but break the
/// contract the operator actually rides on.
#[test]
fn h43_single_symbol_still_resolves_locally() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let file_path = tmp.path().join("single.py");
    std::fs::write(&file_path, FIXTURE_NO_HOMONYM).expect("write fixture");

    let cache = PerFileGraphCache::with_project_dir(tmp.path().to_string_lossy().into_owned());
    let report = cache.merge_with_report(&[file_path.as_path()]);

    let graph: &CallGraph = &report.graph;

    let foo_id = graph
        .symbol_ids()
        .find(|(_, s)| s.name() == "Foo")
        .map(|(id, _)| id.clone())
        .expect("fixture must declare exactly one `Foo`");

    let caller_id = graph
        .symbol_ids()
        .find(|(_, s)| s.name() == "caller")
        .map(|(id, _)| id.clone())
        .expect("fixture must declare exactly one `caller`");

    // The caller invokes `Foo`; the single-symbol resolver must
    // add the edge. Dropping this edge would be a regression
    // of the ordinary intra-file resolution.
    assert!(
        graph
            .dependencies(&caller_id)
            .any(|(t, _kind)| t == &foo_id),
        "single-symbol `Foo` must still be reachable from `caller`; \
         the corrected resolver must not drop the legitimate case"
    );
}
