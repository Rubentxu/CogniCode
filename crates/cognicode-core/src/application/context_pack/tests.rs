//! Behaviour of the context pack over a frozen snapshot.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::*;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::findings::digest::sha256_hex;
use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

struct Files(Mutex<HashMap<String, Vec<u8>>>);

impl Files {
    fn set(&self, relative: &str, text: &str) {
        self.0
            .lock()
            .unwrap()
            .insert(relative.to_string(), text.as_bytes().to_vec());
    }
}

impl SourceReader for Files {
    fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.0
            .lock()
            .unwrap()
            .get(relative)
            .cloned()
            .ok_or_else(|| "missing".to_string())
    }
}

/// Reads a declaration as the line of the symbol and the next two.
struct ThreeLines;

impl DeclarationReader for ThreeLines {
    fn declaration(
        &self,
        _file: &str,
        source: &str,
        line: u32,
        _column: u32,
    ) -> Option<DeclarationSpan> {
        let header = source.lines().nth(line as usize)?.to_string();
        Some(DeclarationSpan {
            start_line: line + 1,
            end_line: line + 3,
            header,
        })
    }
}

struct NoDeclarations;

impl DeclarationReader for NoDeclarations {
    fn declaration(&self, _: &str, _: &str, _: u32, _: u32) -> Option<DeclarationSpan> {
        None
    }
}

struct Ranking(Vec<(SymbolId, f64)>);

impl Relevance for Ranking {
    fn rank(&self, _intent: &str, max: usize) -> Vec<Ranked> {
        self.0
            .iter()
            .take(max)
            .map(|(id, score)| Ranked {
                id: id.clone(),
                score: *score,
            })
            .collect()
    }
}

/// Answers only for the exact query it is given, and records what it was
/// asked: the identifier ranker splits a snake_case identifier into words, so
/// the intent reaches it as one.
struct ForQuery {
    query: &'static str,
    ranking: Vec<(SymbolId, f64)>,
    asked: Mutex<Vec<String>>,
}

impl Relevance for ForQuery {
    fn rank(&self, query: &str, max: usize) -> Vec<Ranked> {
        self.asked.lock().unwrap().push(query.to_string());
        if query != self.query {
            return Vec::new();
        }
        self.ranking
            .iter()
            .take(max)
            .map(|(id, score)| Ranked {
                id: id.clone(),
                score: *score,
            })
            .collect()
    }
}

struct Fixture {
    snapshot: SnapshotView,
    files: Files,
    ids: HashMap<&'static str, SymbolId>,
}

/// Two files declare `parse`; `run` calls the one in `a.rs`.
fn fixture() -> Fixture {
    let a = "fn parse(input: &str) -> u32 {\n    1\n}\nfn run() {\n    parse(\"x\");\n}\n";
    let b = "fn parse(input: &[u8]) -> u32 {\n    2\n}\n";
    let mut graph = CallGraph::new();
    let mut ids = HashMap::new();
    for (key, file, name, line) in [
        ("a_parse", "src/a.rs", "parse", 0),
        ("run", "src/a.rs", "run", 3),
        ("b_parse", "src/b.rs", "parse", 0),
    ] {
        let id = graph.add_symbol(Symbol::new(
            name,
            SymbolKind::Function,
            Location::new(file, line, 3),
        ));
        ids.insert(key, id);
    }
    graph
        .add_dependency(&ids["run"], &ids["a_parse"], DependencyType::Calls)
        .unwrap();
    let files = Files(Mutex::new(HashMap::new()));
    files.set("src/a.rs", a);
    files.set("src/b.rs", b);
    let mut hashes = BTreeMap::new();
    hashes.insert("src/a.rs".to_string(), sha256_hex(a.as_bytes()));
    hashes.insert("src/b.rs".to_string(), sha256_hex(b.as_bytes()));
    let snapshot = SnapshotView {
        graph: Arc::new(graph),
        digest: sha256_hex(b"snapshot-one"),
        graph_source: Some("test".to_string()),
        root: PathBuf::from("/"),
        files: hashes,
    };
    Fixture {
        snapshot,
        files,
        ids,
    }
}

fn pack(
    f: &Fixture,
    relevance: &dyn Relevance,
    decl: &dyn DeclarationReader,
    measure: &dyn TokenMeasure,
    request: PackRequest,
) -> Result<ContextPack, PackError> {
    ContextPackService {
        snapshot: &f.snapshot,
        relevance,
        sources: &f.files,
        declarations: decl,
        measure,
    }
    .build(&request)
}

fn request(anchors: &[&str], max_tokens: u64) -> PackRequest {
    PackRequest {
        selection: SelectionRequest {
            anchors: anchors.iter().map(|a| a.to_string()).collect(),
            intent: None,
            hot_paths: 0,
        },
        max_tokens,
        continuation: None,
    }
}

fn default_pack(f: &Fixture, anchors: &[&str], max_tokens: u64) -> ContextPack {
    pack(
        f,
        &Ranking(vec![]),
        &ThreeLines,
        &Utf8UpperBound,
        request(anchors, max_tokens),
    )
    .unwrap()
}

#[test]
fn a_homonym_is_reported_and_never_resolved_by_bare_name() {
    let f = fixture();
    let p = default_pack(&f, &["parse"], 10_000);
    assert!(
        p.items.is_empty(),
        "no symbol is chosen for an ambiguous name"
    );
    assert_eq!(p.ambiguities.len(), 1);
    let mut expected = vec![f.ids["a_parse"].to_string(), f.ids["b_parse"].to_string()];
    expected.sort();
    assert_eq!(p.ambiguities[0].candidates, expected);
}

#[test]
fn a_symbol_id_names_exactly_one_symbol() {
    let f = fixture();
    let id = f.ids["b_parse"].to_string();
    let p = default_pack(&f, &[&id], 10_000);
    assert_eq!(p.items.len(), 1);
    assert_eq!(p.items[0].id, id);
    assert_eq!(p.items[0].file, "src/b.rs");
    assert!(p.ambiguities.is_empty());
}

#[test]
fn an_item_carries_range_digest_reason_and_the_snapshot_is_named() {
    let f = fixture();
    let id = f.ids["a_parse"].to_string();
    let p = default_pack(&f, &[&id], 10_000);
    let item = &p.items[0];
    assert_eq!(
        item.range,
        SourceRange {
            start_line: 1,
            end_line: 3
        }
    );
    assert_eq!(item.range_basis, "declaration");
    assert_eq!(item.signature, "fn parse(input: &str) -> u32 {");
    assert_eq!(
        item.source_digest,
        format!("sha256:{}", f.snapshot.files["src/a.rs"])
    );
    assert_eq!(item.reason, "anchor");
    assert!(p.rendered.contains(&f.snapshot.digest[..12]));
}

#[test]
fn a_direct_relation_is_selected_with_the_anchor_it_hangs_from() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let p = default_pack(&f, &[&id], 10_000);
    let callee = p
        .items
        .iter()
        .find(|item| item.id == f.ids["a_parse"].to_string())
        .expect("the callee is part of the pack");
    assert_eq!(callee.reason, "callee_of_anchor");
    assert_eq!(callee.reason_detail.as_deref(), Some(id.as_str()));
}

#[test]
fn a_missing_range_is_declared_as_the_line_only() {
    let f = fixture();
    let id = f.ids["a_parse"].to_string();
    let p = pack(
        &f,
        &Ranking(vec![]),
        &NoDeclarations,
        &Utf8UpperBound,
        request(&[&id], 10_000),
    )
    .unwrap();
    assert_eq!(p.items[0].range_basis, "line");
    assert_eq!(p.items[0].range.start_line, 1);
    assert_eq!(p.items[0].range.end_line, 1);
}

#[test]
fn the_rendered_text_never_exceeds_the_limit_under_any_budget() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    for limit in 0..400 {
        let p = default_pack(&f, &[&id], limit);
        let used = Utf8UpperBound.measure(&p.rendered);
        assert!(
            used.tokens <= limit,
            "limit {limit} exceeded: {} tokens in {:?}",
            used.tokens,
            p.rendered
        );
        assert_eq!(p.budget.used.tokens, used.tokens);
        assert!(p.budget.verified);
    }
}

#[test]
fn a_small_budget_bounds_the_output_and_lists_the_omissions() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let all = default_pack(&f, &[&id], 10_000);
    assert_eq!(all.items.len(), 2);
    let first = Utf8UpperBound.measure(&render::render(&render::RenderView {
        snapshot_digest: &f.snapshot.digest,
        items: &all.items[..1],
        remaining: 1,
        refused: 0,
        continuation: Some("v1.0000000000000000.1"),
    }));
    let small = default_pack(&f, &[&id], first.tokens);
    assert_eq!(small.items.len(), 1);
    assert_eq!(small.omitted, 1);
    assert_eq!(small.omissions[0].reason, "budget");
    assert!(small.continuation_token.is_some());
}

#[test]
fn a_budget_too_small_for_one_item_gives_an_empty_text_within_the_limit() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let p = default_pack(&f, &[&id], 3);
    assert!(p.items.is_empty());
    assert!(
        p.continuation_token.is_none(),
        "a page that cannot progress offers no token"
    );
    assert!(p.budget.used.tokens <= 3);
    assert_eq!(p.omitted, p.candidates);
}

#[test]
fn a_default_measure_is_an_upper_bound_and_never_exact() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let p = default_pack(&f, &[&id], 10_000);
    assert!(!p.budget.used.kind.is_exact());
    assert!(matches!(p.budget.used.kind, MeasureKind::UpperBound { .. }));
}

#[test]
fn a_configured_tokenizer_gives_an_exact_count_for_that_tokenizer() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let lexical = measure_named("lexical-v1").unwrap();
    let p = pack(
        &f,
        &Ranking(vec![]),
        &ThreeLines,
        lexical.as_ref(),
        request(&[&id], 10_000),
    )
    .unwrap();
    assert!(p.budget.used.kind.is_exact());
    assert_eq!(p.budget.used, lexical.measure(&p.rendered));
}

#[test]
fn an_unknown_measure_never_claims_the_limit_was_verified() {
    struct Nothing;
    impl TokenMeasure for Nothing {
        fn measure(&self, _: &str) -> Measured {
            Measured {
                tokens: 0,
                kind: MeasureKind::Unknown,
            }
        }
    }
    let f = fixture();
    let id = f.ids["run"].to_string();
    let p = pack(
        &f,
        &Ranking(vec![]),
        &ThreeLines,
        &Nothing,
        request(&[&id], 10_000),
    )
    .unwrap();
    assert!(!p.budget.verified);
}

#[test]
fn pages_cover_every_candidate_once_and_in_order() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let whole = default_pack(&f, &[&id], 100_000);
    let one_item = Utf8UpperBound
        .measure(&render::render(&render::RenderView {
            snapshot_digest: &f.snapshot.digest,
            items: &whole.items[..1],
            remaining: 1,
            refused: 0,
            continuation: Some("v1.0000000000000000.1"),
        }))
        .tokens;
    let mut seen = Vec::new();
    let mut next = None;
    for _ in 0..5 {
        let mut req = request(&[&id], one_item);
        req.continuation = next.clone();
        let p = pack(&f, &Ranking(vec![]), &ThreeLines, &Utf8UpperBound, req).unwrap();
        seen.extend(p.items.iter().map(|i| i.id.clone()));
        next = p.continuation_token.clone();
        if next.is_none() {
            break;
        }
    }
    let expected: Vec<String> = whole.items.iter().map(|i| i.id.clone()).collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_continuation_of_another_snapshot_is_refused() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    let mut req = request(&[&id], 10_000);
    req.continuation = Some(format!("v1.{}.1", "0".repeat(16)));
    let err = pack(&f, &Ranking(vec![]), &ThreeLines, &Utf8UpperBound, req).unwrap_err();
    assert_eq!(err, PackError::StaleContinuation);
}

#[test]
fn a_continuation_that_is_not_ours_is_refused() {
    let f = fixture();
    let mut req = request(&["run"], 10_000);
    req.continuation = Some("garbage".to_string());
    let err = pack(&f, &Ranking(vec![]), &ThreeLines, &Utf8UpperBound, req).unwrap_err();
    assert_eq!(err, PackError::InvalidContinuation);
}

#[test]
fn a_file_changed_since_the_snapshot_is_refused_and_never_mixed_in() {
    let f = fixture();
    f.files.set("src/b.rs", "fn parse() {}\n// edited\n");
    let a = f.ids["a_parse"].to_string();
    let b = f.ids["b_parse"].to_string();
    let p = default_pack(&f, &[&a, &b], 10_000);
    assert!(p.items.iter().all(|item| item.file == "src/a.rs"));
    assert!(p.items.iter().any(|item| item.id == a));
    assert!(
        p.omissions
            .iter()
            .any(|o| o.id == b && o.reason == "source_changed")
    );
    assert_eq!(p.omitted, 1);
}

#[test]
fn an_unreadable_file_is_an_omission_with_its_reason() {
    let f = fixture();
    f.files.0.lock().unwrap().remove("src/b.rs");
    let b = f.ids["b_parse"].to_string();
    let p = default_pack(&f, &[&b], 10_000);
    assert!(p.items.is_empty());
    assert_eq!(p.omissions[0].reason, "unreadable");
}

#[test]
fn the_same_request_over_the_same_snapshot_gives_the_same_pack() {
    let f = fixture();
    let id = f.ids["run"].to_string();
    assert_eq!(default_pack(&f, &[&id], 500), default_pack(&f, &[&id], 500));
}

#[test]
fn an_anchor_that_names_nothing_is_unresolved() {
    let f = fixture();
    let p = default_pack(&f, &["nowhere"], 10_000);
    assert_eq!(p.unresolved, vec!["nowhere".to_string()]);
}

#[test]
fn the_intent_ranking_contributes_name_relevance_candidates() {
    let f = fixture();
    let run = f.ids["run"].clone();
    let req = PackRequest {
        selection: SelectionRequest {
            anchors: vec![],
            intent: Some("go".to_string()),
            hot_paths: 0,
        },
        max_tokens: 10_000,
        continuation: None,
    };
    let p = pack(
        &f,
        &Ranking(vec![(run.clone(), 2.5)]),
        &ThreeLines,
        &Utf8UpperBound,
        req,
    )
    .unwrap();
    assert_eq!(p.items.len(), 1);
    assert_eq!(p.items[0].reason, "name_relevance");
    assert_eq!(p.items[0].score, 2.5);
}

#[test]
fn a_phrase_intent_reaches_the_ranker_as_one_identifier_and_keeps_its_order() {
    let f = fixture();
    let (run, a, b) = (
        f.ids["run"].clone(),
        f.ids["a_parse"].clone(),
        f.ids["b_parse"].clone(),
    );
    let ranking = ForQuery {
        query: "parse_then_run",
        ranking: vec![(run.clone(), 1.5), (b.clone(), 1.0), (a.clone(), 2.0)],
        asked: Mutex::new(Vec::new()),
    };
    let selection = select(
        &f.snapshot.graph,
        &ranking,
        &SelectionRequest {
            anchors: vec![],
            intent: Some("Parse, then run".to_string()),
            hot_paths: 0,
        },
    );

    assert_eq!(*ranking.asked.lock().unwrap(), vec!["parse_then_run"]);
    let order: Vec<&SymbolId> = selection.candidates.iter().map(|c| &c.id).collect();
    assert_eq!(order, vec![&a, &run, &b], "best score first");
    assert!(
        selection
            .candidates
            .iter()
            .all(|c| c.reason.code() == "name_relevance"),
        "no name is the whole phrase"
    );
}

#[test]
fn a_symbol_named_like_the_phrase_is_an_exact_match_in_snake_or_camel_case() {
    let f = fixture();
    let run = f.ids["run"].clone();
    let ranking = ForQuery {
        query: "run",
        ranking: vec![(run.clone(), 9.0)],
        asked: Mutex::new(Vec::new()),
    };
    let selection = select(
        &f.snapshot.graph,
        &ranking,
        &SelectionRequest {
            anchors: vec![],
            intent: Some("  Run  ".to_string()),
            hot_paths: 0,
        },
    );
    assert_eq!(selection.candidates[0].reason.code(), "exact_name");
}

#[test]
fn the_zero_query_selection_proposes_the_most_connected_symbols_with_their_reason() {
    let mut graph = CallGraph::new();
    let mut add = |name: &str, line: u32| {
        graph.add_symbol(Symbol::new(
            name,
            SymbolKind::Function,
            Location::new("src/a.rs", line, 0),
        ))
    };
    let hot = add("hot", 0);
    let warm = add("warm", 1);
    let c1 = add("c1", 2);
    let c2 = add("c2", 3);
    let c3 = add("c3", 4);
    for (from, to) in [
        (&c1, &hot),
        (&c2, &hot),
        (&c3, &hot),
        (&c1, &warm),
        (&c2, &warm),
    ] {
        graph
            .add_dependency(from, to, DependencyType::Calls)
            .unwrap();
    }
    let request = SelectionRequest {
        hot_paths: 10,
        ..SelectionRequest::default()
    };
    let selection = select(&graph, &NoRelevance, &request);
    let ids: Vec<_> = selection.candidates.iter().map(|c| c.id.clone()).collect();
    assert_eq!(
        ids,
        vec![hot, warm],
        "fan_in 3 then 2; one caller is not hot"
    );
    assert_eq!(selection.candidates[0].reason.code(), "hot_path");
    assert_eq!(
        selection.candidates[0].reason.detail().as_deref(),
        Some("fan_in=3, fan_out=0")
    );
    assert!(
        selection.candidates.iter().all(|c| c.score == 1.0),
        "called and calling nothing scores one"
    );

    let capped = select(
        &graph,
        &NoRelevance,
        &SelectionRequest {
            hot_paths: 1,
            ..SelectionRequest::default()
        },
    );
    assert_eq!(capped.candidates.len(), 1);
    assert!(
        select(&graph, &NoRelevance, &SelectionRequest::default())
            .candidates
            .is_empty(),
        "a request that asks for nothing selects nothing"
    );
}
