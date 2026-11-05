//! The product publishes every line with one base: one, and the core
//! prints no answer of its own.
//!
//! ## What this measures
//!
//! Two contracts that the CLI's file
//! (`crates/cognicode-cli/tests/it/one_published_line_base.rs`)
//! measures from the other side.
//!
//! **The line.** A line number is a position, and a position is only a
//! position if everybody counts the same way. The graph's positions
//! are zero-indexed (that is what the parser hands out) and the
//! code-intelligence positions are one-indexed (that is what the
//! product publishes), and the surfaces that published a graph
//! position published it zero-indexed while the surface next to it
//! published one-indexed. The contract: the published line is the line
//! the file declares — the one a person sees in their editor.
//!
//! **The printing.** The core returns results; the presenter prints
//! them. A core that writes to the standard output can interleave its
//! own text with the answer the consumer is parsing, and the only
//! place that may write there is the channel rule: the progress line,
//! in text mode.

use std::path::{Path, PathBuf};

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph, handle_get_entry_points,
};
use cognicode_core::interface::mcp::schemas::GetEntryPointsInput;

/// A file whose declarations are on known lines.
///
/// `alpha` is declared on line 1 and `beta` on line 5.
const FIXTURE: &str = "\
pub fn alpha() -> u32 {
    beta()
}

pub fn beta() -> u32 {
    2
}
";

/// A workspace with one file, whose declarations are on known lines.
fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-core-one-line-base-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("the fixture directory");
    std::fs::write(dir.join("lib.rs"), FIXTURE).expect("the fixture file");
    dir
}

/// The line the file declares the symbol on.
fn declared_line(symbol: &str) -> Option<u32> {
    FIXTURE
        .lines()
        .position(|line| line.contains(&format!("fn {symbol}(")))
        .map(|index| index as u32 + 1)
}

/// The MCP publishes the line the file declares.
///
/// The entry points of the fixture are the functions nobody calls, and
/// the answer says where each one lives. The line it says is the line
/// of the file.
#[tokio::test]
async fn the_mcp_publishes_the_line_the_file_declares() {
    let dir = fixture();
    let ctx = HandlerContext::builder()
        .with_working_dir(dir.clone())
        .build();
    handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP builds the graph");

    let answer = handle_get_entry_points(&ctx, GetEntryPointsInput { compressed: false })
        .await
        .expect("the MCP answers with the entry points");

    assert!(
        !answer.entry_points.is_empty(),
        "the fixture declares an entry point"
    );
    for entry in &answer.entry_points {
        let Some(declared) = declared_line(&entry.name) else {
            continue;
        };
        assert_eq!(
            entry.location.line, declared,
            "the published line of {} is not the line the file declares it on",
            entry.name
        );
    }
}

/// The core's CLI module writes to the standard output in one place
/// only: the channel rule.
///
/// The rule is `interface::cli::progress::progress_line` — the
/// progress line, in text mode. Everything else the core returns, and
/// the presenter prints it. A core that prints its own answer can
/// interleave it with the document a consumer is parsing, and the
/// symptom is a JSON document that is not a document.
///
/// The check reads the sources, so it holds for the arms nobody
/// exercised: a `println!` added to a command that has no test is
/// still a `println!`.
#[test]
fn the_core_prints_only_through_the_channel_rule() {
    let cli_module = repo_root().join("crates/cognicode-core/src/interface/cli");
    let mut offenders: Vec<String> = Vec::new();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&cli_module)
        .expect("the CLI module directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();

    for file in files {
        // The channel rule is the one place that writes there.
        if file.file_name().is_some_and(|name| name == "progress.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("the source file");
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // `eprintln!` is the observability channel and is allowed;
            // `println!` is the product's channel and is not.
            if trimmed.starts_with("println!") {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.file_name().unwrap().to_string_lossy(),
                    number + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the core prints to the standard output outside the channel rule: {}",
        offenders.join("; ")
    );
}

/// The root of the repository, found by walking up from this file.
fn repo_root() -> PathBuf {
    let mut path = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    while !path.join("Cargo.toml").exists() || !path.join("crates").is_dir() {
        assert!(
            path.parent().is_some(),
            "the repository root is above the manifest"
        );
        path = path.parent().unwrap().to_path_buf();
    }
    path
}

/// The published schema says which base every published line uses.
///
/// A schema that publishes a line and does not say how it counts is a
/// schema the consumer has to guess at, and the guess is the defect this
/// task closes: two surfaces, two bases, and nothing that said so.
///
/// The contract: every line the product publishes is counted from one,
/// and the schema says so in the property that carries it.
///
/// The two halves are read from the two places the schema comes from:
/// the input schema is hand-authored in `product/tools.json` — the
/// master the runtime publishes — and the output schema is derived
/// from the type that serialises the answer.
#[test]
fn the_published_schema_documents_the_line_base() {
    let mut offenders: Vec<String> = Vec::new();

    // The inputs: the master catalog, hand-authored.
    let master = read_json(&repo_root().join("product").join("tools.json"));
    for tool in master["tools"].as_array().into_iter().flatten() {
        let Some(name) = tool["name"].as_str() else {
            continue;
        };
        let Some(properties) = tool["input_schema"]["properties"].as_object() else {
            continue;
        };
        for (field, spec) in properties {
            if !is_a_line(field) {
                continue;
            }
            check_description(&mut offenders, &format!("{name}.{field}"), spec);
        }
    }

    // The outputs: derived from the type that produces the answer.
    for (tool, schema) in
        cognicode_core::interface::mcp::output_contracts::published_output_schemas()
    {
        collect_output_lines(
            &mut offenders,
            tool,
            &serde_json::Value::Object(schema.clone()),
        );
    }

    assert!(
        offenders.is_empty(),
        "the published schema publishes a line without saying how it counts: {}",
        offenders.join("; ")
    );
}

/// Whether a property carries a line.
fn is_a_line(field: &str) -> bool {
    matches!(field, "line" | "start_line" | "end_line")
}

/// Check that a property says the line counts from one.
fn check_description(offenders: &mut Vec<String>, path: &str, spec: &serde_json::Value) {
    let description = spec["description"].as_str();
    match description {
        None => offenders.push(format!("{path} says nothing about its base")),
        Some(text) if !text.contains("1-indexed") => offenders.push(format!(
            "{path} says {text:?}, which does not name the base"
        )),
        Some(_) => {}
    }
}

/// Walk a derived output schema and check every line property in it.
fn collect_output_lines(offenders: &mut Vec<String>, tool: &str, schema: &serde_json::Value) {
    let Some(properties) = schema["properties"].as_object() else {
        return;
    };
    for (field, spec) in properties {
        if is_a_line(field) {
            check_description(offenders, &format!("{tool}.{field}"), spec);
        }
    }
    // A schema that names its shapes in `$defs` carries lines there too.
    if let Some(defs) = schema["$defs"].as_object() {
        for (name, def) in defs {
            collect_output_lines(offenders, &format!("{tool}.{name}"), def);
        }
    }
}

/// Read a JSON document, or fail with the path that could not be read.
fn read_json(path: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}
