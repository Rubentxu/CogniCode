//! The goal `pack` over real files, in the languages the fixtures cover.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::application::context_pack::TokenMeasure;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn run_pack(root: &Path, arguments: Value) -> Result<Value, String> {
    run(&GoalEnvironment::new(root, None), &arguments)
}

fn rust_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/a.rs",
        "pub fn parse(input: &str) -> u32 {\n    input.len() as u32\n}\n\npub fn run() -> u32 {\n    parse(\"x\")\n}\n",
    );
    write(
        dir.path(),
        "src/b.rs",
        "pub fn parse(input: &[u8]) -> u32 {\n    input.len() as u32\n}\n",
    );
    dir
}

fn ids_named(answer: &Value, name: &str) -> Vec<String> {
    answer["ambiguities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["query"] == name)
        .map(|a| {
            a["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_rust_homonym_is_ambiguous_and_its_id_resolves_with_evidence() {
    let dir = rust_project();
    let answer = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&answer, "parse");
    assert_eq!(candidates.len(), 2, "answer: {answer}");
    assert!(answer["items"].as_array().unwrap().is_empty());
    assert_eq!(answer["status"], "partial");

    let one = run_pack(dir.path(), json!({"symbols": [candidates[0]]})).unwrap();
    let item = &one["items"][0];
    assert_eq!(item["id"], candidates[0]);
    assert_eq!(item["reason"], "anchor");
    assert_eq!(item["range_basis"], "declaration");
    assert!(
        item["source_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        item["signature"]
            .as_str()
            .unwrap()
            .starts_with("pub fn parse(")
    );
    assert!(item["range"]["end_line"].as_u64() >= item["range"]["start_line"].as_u64());
    assert_eq!(one["snapshot"]["files"], 2);
    assert_eq!(one["semantic_coverage"]["assessed"], false);
}

#[test]
fn the_text_the_budget_measured_is_the_text_the_goal_renders() {
    let dir = rust_project();
    let answer = run_pack(dir.path(), json!({"intent": "parse", "max_tokens": 400})).unwrap();
    let text = render(&answer);
    let used = answer["budget"]["used"]["tokens"].as_u64().unwrap();
    assert_eq!(used, Utf8UpperBound.measure(&text).tokens);
    assert!(used <= 400);
    assert_eq!(answer["budget"]["used"]["kind"], "upper_bound");
    assert_eq!(answer["budget"]["used"]["method"], "utf8-bytes");
    assert_eq!(answer["budget"]["verified"], true);
    assert_eq!(answer["budget"]["scope"], "rendered_text");
}

#[test]
fn a_java_homonym_is_ambiguous_by_name_and_exact_by_id() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/A.java",
        "public class A {\n    public int parse(String s) {\n        return s.length();\n    }\n}\n",
    );
    write(
        dir.path(),
        "src/B.java",
        "public class B {\n    public int parse(byte[] s) {\n        return s.length;\n    }\n}\n",
    );
    let answer = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&answer, "parse");
    assert_eq!(candidates.len(), 2, "answer: {answer}");
    let one = run_pack(dir.path(), json!({"symbols": [candidates[1]]})).unwrap();
    assert_eq!(one["items"][0]["id"], candidates[1]);
    assert!(
        one["items"][0]["file"]
            .as_str()
            .unwrap()
            .ends_with("B.java")
    );
}

#[test]
fn a_kotlin_homonym_is_ambiguous_by_name_and_exact_by_id() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/a.kt",
        "fun parse(input: String): Int {\n    return input.length\n}\n",
    );
    write(
        dir.path(),
        "src/b.kt",
        "fun parse(input: ByteArray): Int {\n    return input.size\n}\n",
    );
    let answer = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&answer, "parse");
    assert_eq!(candidates.len(), 2, "answer: {answer}");
    let one = run_pack(dir.path(), json!({"symbols": [candidates[0]]})).unwrap();
    assert_eq!(one["items"][0]["id"], candidates[0]);
}

#[test]
fn a_tokenizer_that_is_not_shipped_is_refused() {
    let dir = rust_project();
    let error = run_pack(
        dir.path(),
        json!({"symbols": ["run"], "tokenizer": "cl100k"}),
    )
    .unwrap_err();
    assert!(error.contains("lexical-v1"));
}

#[test]
fn a_request_without_symbols_or_intent_is_refused() {
    let dir = rust_project();
    assert!(run_pack(dir.path(), json!({})).is_err());
}

#[test]
fn a_page_token_of_another_snapshot_is_refused() {
    let dir = rust_project();
    let probe = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&probe, "parse");
    let both = json!([candidates[0], candidates[1]]);
    let token = (40..600)
        .step_by(20)
        .find_map(|limit| {
            let page = run_pack(dir.path(), json!({"symbols": both, "max_tokens": limit})).ok()?;
            page["continuation_token"].as_str().map(str::to_string)
        })
        .expect("some budget cuts the pack and gives a token");

    write(dir.path(), "src/b.rs", "pub fn parse() {}\n");
    let error = run_pack(
        dir.path(),
        json!({"symbols": both, "max_tokens": 40, "continuation_token": token}),
    )
    .unwrap_err();
    assert!(error.contains("another snapshot"), "{error}");
}

#[test]
fn pages_of_one_snapshot_deliver_every_candidate_once() {
    let dir = rust_project();
    let probe = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&probe, "parse");
    let both = json!([candidates[0], candidates[1]]);
    let whole = run_pack(dir.path(), json!({"symbols": both, "max_tokens": 100000})).unwrap();
    let expected: Vec<Value> = whole["items"].as_array().unwrap().clone();
    assert!(expected.len() >= 2);

    let mut seen: Vec<Value> = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..10 {
        let mut arguments = json!({"symbols": both, "max_tokens": 600});
        if let Some(token) = &token {
            arguments["continuation_token"] = json!(token);
        }
        let page = run_pack(dir.path(), arguments).unwrap();
        seen.extend(page["items"].as_array().unwrap().iter().cloned());
        token = page["continuation_token"].as_str().map(str::to_string);
        if token.is_none() {
            break;
        }
    }
    assert_eq!(seen, expected);
}

#[test]
fn a_budget_that_fits_no_candidate_says_so_and_offers_no_page_to_loop_on() {
    let dir = rust_project();
    let probe = run_pack(dir.path(), json!({"symbols": ["parse"]})).unwrap();
    let candidates = ids_named(&probe, "parse");
    let page = run_pack(
        dir.path(),
        json!({"symbols": [candidates[0]], "max_tokens": 5}),
    )
    .unwrap();
    assert!(page["items"].as_array().unwrap().is_empty());
    assert_eq!(page["truncated"], true);
    assert_eq!(page["truncation_reason"], "budget_too_small");
    assert!(page.get("continuation_token").is_none());
    assert!(page["budget"]["used"]["tokens"].as_u64().unwrap() <= 5);
}

#[test]
fn the_coverage_is_a_bounded_sample_with_exact_totals() {
    use crate::interface::coverage::PendingReason;

    let pending: Vec<PendingReferenceInfo> = (0..45)
        .map(|n| PendingReferenceInfo {
            file: format!("src/f{n}.rs"),
            line: Some(1),
            target: format!("t{n}"),
            kind: "call".to_string(),
            reason: PendingReason::Unresolved,
            candidates: None,
        })
        .collect();
    let coverage = PackCoverage::bounded(CoverageInfo {
        status: "partial".to_string(),
        skipped_files: vec![],
        pending_references: pending,
        rejected_relations: vec![],
    });

    assert_eq!(coverage.pending_references_total, 45);
    assert_eq!(coverage.pending_references.len(), COVERAGE_LISTED);
    assert_eq!(
        coverage.pending_references[0].target, "t0",
        "the first ones"
    );
    assert_eq!(coverage.skipped_files_total, 0);
    assert_eq!(coverage.status, "partial");
}

mod reuse_within_a_session {
    //! Inside a session the pack shares the published graph and its ranking
    //! statistics between calls, and never serves them past a change of the
    //! tree.

    use std::sync::Arc;

    use super::*;
    use crate::application::services::analysis_service::AnalysisService;
    use crate::domain::traits::graph_store::GraphStore;
    use crate::infrastructure::graph::analytics::ranking_stats::RankingStatsCache;
    use crate::infrastructure::persistence::CachedGraphStore;
    use crate::interface::project_graph::GraphSession;

    fn session() -> GraphSession {
        let analysis = Arc::new(AnalysisService::new());
        let store: Arc<dyn GraphStore> = Arc::new(CachedGraphStore::new(analysis.graph_cache()));
        GraphSession::new(analysis, store, Arc::new(RankingStatsCache::default()))
    }

    fn pack_text(environment: &GoalEnvironment, arguments: &Value) -> String {
        serde_json::to_string(&run(environment, arguments).unwrap()).unwrap()
    }

    fn request() -> Value {
        json!({"intent": "parse run", "max_tokens": 400})
    }

    /// A project with a durable snapshot already published, so that a fresh
    /// build and a session answer from the same source of the graph.
    fn warm_project() -> (tempfile::TempDir, tempfile::TempDir) {
        let project = rust_project();
        let cache = tempfile::tempdir().unwrap();
        run(
            &GoalEnvironment::new(project.path(), Some(cache.path())),
            &request(),
        )
        .unwrap();
        (project, cache)
    }

    #[test]
    fn a_session_pack_is_byte_identical_to_a_fresh_one() {
        let (project, cache) = warm_project();
        let session = session();
        let fresh = pack_text(
            &GoalEnvironment::new(project.path(), Some(cache.path())),
            &request(),
        );

        let environment =
            GoalEnvironment::new(project.path(), Some(cache.path())).with_session(&session);
        for call in 0..3 {
            assert_eq!(
                pack_text(&environment, &request()),
                fresh,
                "call {call} of the session"
            );
        }
    }

    #[test]
    fn repeated_calls_share_the_graph_and_build_the_statistics_once() {
        let (project, cache) = warm_project();
        let session = session();
        let environment =
            GoalEnvironment::new(project.path(), Some(cache.path())).with_session(&session);

        let first = session
            .project_graph(project.path(), Some(cache.path()))
            .unwrap();
        for _ in 0..4 {
            run(&environment, &request()).unwrap();
        }
        let last = session
            .project_graph(project.path(), Some(cache.path()))
            .unwrap();

        assert!(
            Arc::ptr_eq(&first.graph, &last.graph),
            "every call hands out the snapshot the session published"
        );
        let counters = session.ranking_stats().counters();
        assert_eq!(counters.builds, 1, "counters: {counters:?}");
        assert!(counters.hits >= 3, "counters: {counters:?}");
    }

    #[test]
    fn a_later_call_keeps_the_coverage_the_build_measured() {
        let project = tempfile::tempdir().unwrap();
        write(
            project.path(),
            "src/a.rs",
            "pub fn run() -> u32 {\n    parse(1) + helper()\n}\n\npub fn helper() -> u32 {\n    missing_callee()\n}\n",
        );
        write(
            project.path(),
            "src/b.rs",
            "pub fn parse(x: u32) -> u32 {\n    x\n}\n",
        );
        write(
            project.path(),
            "src/c.rs",
            "pub fn parse(x: u64) -> u64 {\n    x\n}\n",
        );
        let session = session();
        let environment = GoalEnvironment::new(project.path(), None).with_session(&session);

        let built = run(&environment, &request()).unwrap();
        let served = run(&environment, &request()).unwrap();

        assert_eq!(built["basis"]["graph_source"], "built");
        assert_eq!(served["basis"]["graph_source"], "cache");
        assert!(
            built["coverage"]["pending_references_total"]
                .as_u64()
                .unwrap()
                > 0,
            "the fixture leaves references pending: {built}"
        );
        assert_eq!(
            built["coverage"], served["coverage"],
            "serving the graph again does not forget what its build left open"
        );
    }

    #[test]
    fn a_change_of_the_tree_is_never_answered_from_the_old_graph() {
        let (project, cache) = warm_project();
        let session = session();
        let environment =
            GoalEnvironment::new(project.path(), Some(cache.path())).with_session(&session);
        let before = run(&environment, &request()).unwrap();

        write(
            project.path(),
            "src/c.rs",
            "pub fn parse_more() -> u32 {\n    7\n}\n",
        );
        let after = run(&environment, &request()).unwrap();
        let fresh = run(
            &GoalEnvironment::new(project.path(), Some(cache.path())),
            &request(),
        )
        .unwrap();

        assert_ne!(
            before["snapshot"]["digest"], after["snapshot"]["digest"],
            "the digest names the new tree"
        );
        assert_eq!(after["snapshot"]["files"], 3);
        // Only the provenance label may differ: the session rebuilt the graph
        // and the fresh call then read the snapshot the rebuild published.
        let without_source = |mut answer: Value| {
            answer["basis"]
                .as_object_mut()
                .unwrap()
                .remove("graph_source");
            answer
        };
        assert_eq!(
            without_source(after),
            without_source(fresh),
            "the session answers what a fresh build does"
        );
        assert_eq!(
            session.ranking_stats().counters().builds,
            2,
            "the statistics of the new snapshot are built, not reused"
        );
    }
}

mod adaptive {
    use super::*;

    fn chain_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "src/lib.rs",
            "pub fn load(path: &str) -> u32 {\n    decode(path)\n}\n\n\
             pub fn decode(text: &str) -> u32 {\n    checksum(text)\n}\n\n\
             pub fn checksum(text: &str) -> u32 {\n    text.len() as u32\n}\n\n\
             pub fn report() -> u32 {\n    load(\"a\")\n}\n",
        );
        dir
    }

    #[test]
    fn a_plain_pack_publishes_no_adaptive_report() {
        let dir = chain_project();

        let answer = run_pack(dir.path(), json!({"symbols": ["decode"]})).unwrap();

        assert!(answer.get("adaptive").is_none(), "{answer}");
        assert!(!render(&answer).contains("selection: mode="));
    }

    #[test]
    fn a_mode_publishes_its_policy_and_the_text_carries_the_same_note() {
        let dir = chain_project();

        let answer = run_pack(
            dir.path(),
            json!({"symbols": ["decode"], "mode": "diagnose", "max_tokens": 4000}),
        )
        .unwrap();

        let report = &answer["adaptive"];
        assert_eq!(report["policy"]["mode"], "diagnose");
        assert_eq!(report["expansion"]["status"], "expanded", "{answer}");
        assert!(
            report["score_semantics"]
                .as_str()
                .unwrap()
                .contains("not a confidence"),
            "{report}"
        );
        let text = render(&answer);
        assert!(text.contains("selection: mode=diagnose"), "{text}");
        let used = answer["budget"]["used"]["tokens"].as_u64().unwrap();
        assert_eq!(
            used,
            Utf8UpperBound.measure(&text).tokens,
            "the budget measures the text the goal renders, notes included"
        );
        assert!(
            answer["summary"]
                .as_str()
                .unwrap()
                .starts_with("mode diagnose, expansion expanded:")
        );
        let ids: Vec<&str> = answer["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        assert!(ids.len() >= 2, "the walk adds the neighbours: {ids:?}");
        assert!(
            ids[0].contains("decode"),
            "the named symbol goes first: {ids:?}"
        );
    }

    #[test]
    fn the_same_request_gives_the_same_answer() {
        let dir = chain_project();
        let request = json!({"symbols": ["decode"], "mode": "refactor", "max_tokens": 4000});

        assert_eq!(
            run_pack(dir.path(), request.clone()).unwrap(),
            run_pack(dir.path(), request).unwrap()
        );
    }

    #[test]
    fn expansion_never_abstains_and_says_so() {
        let dir = chain_project();

        let answer = run_pack(
            dir.path(),
            json!({"symbols": ["decode"], "mode": "diagnose", "expansion": "never"}),
        )
        .unwrap();

        assert_eq!(answer["adaptive"]["expansion"]["status"], "abstained");
        assert_eq!(answer["items"].as_array().unwrap().len(), 1, "{answer}");
    }

    #[test]
    fn a_walk_a_bound_stopped_makes_the_pack_partial() {
        let dir = chain_project();

        let answer = run_pack(
            dir.path(),
            json!({
                "symbols": ["decode"], "mode": "diagnose",
                "expansion": "always", "max_visits": 1, "max_tokens": 4000
            }),
        )
        .unwrap();

        assert_eq!(
            answer["adaptive"]["expansion"]["status"], "partial",
            "{answer}"
        );
        assert_eq!(answer["status"], "partial");
        assert_eq!(answer["selection"]["partial"], true);
    }

    #[test]
    fn the_detail_of_an_adaptive_pack_can_be_forced() {
        let dir = chain_project();

        let answer = run_pack(
            dir.path(),
            json!({"symbols": ["decode"], "mode": "diagnose", "detail": "signature"}),
        )
        .unwrap();

        assert_eq!(answer["adaptive"]["policy"]["detail_override"], "signature");
        for item in answer["items"].as_array().unwrap() {
            assert_eq!(item["representation"]["level"], "signature", "{item}");
        }
    }

    #[test]
    fn a_tuning_without_a_mode_and_a_mode_with_a_change_are_refused() {
        let dir = chain_project();

        let stray = run_pack(dir.path(), json!({"symbols": ["decode"], "detail": "full"}));
        let change = run_pack(
            dir.path(),
            json!({"mode": "diagnose", "change_scope": "staged"}),
        );

        assert!(stray.unwrap_err().contains("pass `mode`"));
        assert!(change.unwrap_err().contains("change_scope"));
    }

    #[test]
    fn a_page_token_belongs_to_the_mode_that_made_it() {
        let dir = chain_project();
        let token = (60..1200)
            .step_by(20)
            .find_map(|limit| {
                let page = run_pack(
                    dir.path(),
                    json!({"symbols": ["decode"], "mode": "diagnose", "max_tokens": limit}),
                )
                .ok()?;
                page["continuation_token"].as_str().map(str::to_string)
            })
            .expect("some budget cuts the pack and gives a token");

        let other = run_pack(
            dir.path(),
            json!({
                "symbols": ["decode"], "mode": "refactor",
                "max_tokens": 60, "continuation_token": token
            }),
        );

        assert!(other.is_err(), "a token of another policy is not accepted");
    }
}
