//! What a pack says about going further.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::*;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |relative: &str, text: &str| {
        let path = dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(
        "src/lib.rs",
        "pub fn leaf() -> u32 {\n    1\n}\n\npub fn left() -> u32 {\n    leaf()\n}\n\npub fn right() -> u32 {\n    leaf() + 1\n}\n",
    );
    dir
}

fn pack_of(root: &Path, arguments: Value) -> Value {
    run(&GoalEnvironment::new(root, None), &arguments).unwrap()
}

fn id_of(answer: &Value, name: &str) -> String {
    answer["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("no item named {name}: {answer}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn each_delivered_symbol_states_what_expanding_it_reaches() {
    let dir = project();
    let answer = pack_of(dir.path(), json!({"intent": "leaf left right"}));
    let leaf = id_of(&answer, "leaf");
    let reach = answer["navigation"]["reach"]
        .as_array()
        .unwrap()
        .iter()
        .find(|reach| reach["id"] == leaf)
        .unwrap();
    assert_eq!(reach["direct_callers"], 2, "answer: {answer}");
    assert_eq!(reach["direct_callees"], 0);
    assert_eq!(reach["range_lines"], 3);
    assert_eq!(
        answer["navigation"]["reach"].as_array().unwrap().len(),
        answer["items"].as_array().unwrap().len()
    );
}

#[test]
fn a_whole_page_may_stop_but_never_certifies_the_code_it_did_not_select() {
    let dir = project();
    let answer = pack_of(dir.path(), json!({"intent": "leaf", "max_tokens": 4000}));
    let advisory = &answer["navigation"]["advisory"];
    assert_eq!(advisory["verdict"], "may_stop", "answer: {answer}");
    assert_eq!(advisory["basis"], "advisory");
    assert!(advisory["gaps"].as_array().unwrap().is_empty());
    let limits = advisory["limits"].as_array().unwrap();
    assert!(
        limits
            .iter()
            .any(|limit| limit.as_str().unwrap().contains("not assessed"))
    );
    assert_eq!(answer["semantic_coverage"]["assessed"], false);
    assert_ne!(answer["status"], "complete");
}

#[test]
fn a_cut_page_keeps_going_and_names_why() {
    let dir = project();
    let answer = pack_of(
        dir.path(),
        json!({"intent": "leaf left right", "max_tokens": 60}),
    );
    assert_eq!(answer["truncated"], true, "answer: {answer}");
    let advisory = &answer["navigation"]["advisory"];
    assert_eq!(advisory["verdict"], "keep_going");
    let codes: Vec<&str> = advisory["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"candidates_left"), "codes: {codes:?}");
}

#[test]
fn an_unresolved_anchor_keeps_going_and_delivers_nothing_to_expand() {
    let dir = project();
    let answer = pack_of(dir.path(), json!({"symbols": ["missing"]}));
    assert!(answer["navigation"]["reach"].as_array().unwrap().is_empty());
    let advisory = &answer["navigation"]["advisory"];
    assert_eq!(advisory["verdict"], "keep_going");
    let codes: Vec<&str> = advisory["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"nothing_delivered"));
    assert!(codes.contains(&"unresolved_anchor"));
}
