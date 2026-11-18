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
    assert!(item["source_digest"].as_str().unwrap().starts_with("sha256:"));
    assert!(item["signature"].as_str().unwrap().starts_with("pub fn parse("));
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
    assert!(one["items"][0]["file"].as_str().unwrap().ends_with("B.java"));
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
    let error = run_pack(dir.path(), json!({"symbols": ["run"], "tokenizer": "cl100k"}))
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
    let page = run_pack(dir.path(), json!({"symbols": [candidates[0]], "max_tokens": 5})).unwrap();
    assert!(page["items"].as_array().unwrap().is_empty());
    assert_eq!(page["truncated"], true);
    assert_eq!(page["truncation_reason"], "budget_too_small");
    assert!(page.get("continuation_token").is_none());
    assert!(page["budget"]["used"]["tokens"].as_u64().unwrap() <= 5);
}
