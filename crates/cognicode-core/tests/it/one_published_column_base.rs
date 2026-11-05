//! The product publishes every column with one base: one.
//!
//! ## What this measures
//!
//! A line number is a position, and a position is only a position if
//! everybody counts the same way. The line found its base in the
//! product's own hands; the column was left behind, and it was left
//! behind in two ways at once.
//!
//! **The published output.** The graph counts from zero — that is what
//! the parser hands out — and the surfaces that publish a graph
//! position published the column next to the line as the graph carried
//! it, while the line next to it was already the one a person reads.
//! Two numbers in one object, in two bases.
//!
//! **The published input.** `get_symbol_code` took the column the way
//! the parser counts it and the catalogue said so: "Column number
//! (0-indexed)". Meanwhile the code-intelligence tools — `hover`,
//! `go_to_definition`, `find_references`, `rename_preview` — took the
//! column the way a person reads it, and the catalogue said nothing.
//!
//! The contract: the published column is the column the file declares,
//! the same way the published line is the line the file declares, and
//! the catalogue says so wherever a column appears.

use std::path::PathBuf;

use cognicode_core::infrastructure::semantic::SymbolCodeService;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph, handle_get_call_hierarchy,
    handle_get_entry_points, handle_get_hot_paths,
};
use cognicode_core::interface::mcp::schemas::{
    CallDirection, GetCallHierarchyInput, GetEntryPointsInput, GetHotPathsInput,
};

/// A file with two declarations, so that the column says something.
///
/// Both declarations are on the same line, and the second one does not
/// start at the first character: a column of zero would point at the
/// character before the declaration, which is nobody's column.
const FIXTURE: &str = "fn alpha() { beta(); } fn beta() {}\n";

/// The catalogue the product publishes, and the capture of what the
/// runtime publishes.
const CATALOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../product/tools.json");
const CAPTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../product/tool-catalog-runtime.json"
);

/// A workspace with one file, whose declarations are on known lines.
fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-core-one-column-base-{}-{}",
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

/// The column the file declares the symbol on.
///
/// The declaration starts at its first character, and the published
/// column is the one that points at it: the column a person reads.
/// A column that counts from zero points at the character before the
/// declaration.
fn declared_column(symbol: &str) -> Option<u32> {
    let line = declared_line(symbol)?;
    let text = FIXTURE.lines().nth(line as usize - 1)?;
    let at = text.find(&format!("fn {symbol}("))?;
    Some(at as u32 + 1)
}

/// The character the file declares at that position.
fn character_at(line: u32, column: u32) -> Option<char> {
    let text = FIXTURE.lines().nth(line as usize - 1)?;
    text.chars().nth(column as usize - 1)
}

/// The MCP publishes the column the file declares.
///
/// The entry points of the fixture are the functions nobody calls, and
/// the answer says where each one lives. The column it says is the
/// column of the file.
#[tokio::test]
async fn the_mcp_publishes_the_column_the_file_declares() {
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
        let Some(declared) = declared_column(&entry.name) else {
            continue;
        };
        assert_eq!(
            entry.location.column, declared,
            "the published column of {} is not the column the file declares",
            entry.name
        );
    }
}

/// The graph tools publish the column the file declares.
///
/// The graph counts from zero; the product counts from one. Every tool
/// that answers with a graph position has to cross that difference
/// before it publishes, and each one that forgets answers with the
/// column of the character before the declaration.
#[tokio::test]
async fn the_graph_tools_publish_the_column_the_file_declares() {
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

    // The hot paths say where each hot symbol lives.
    let hot = handle_get_hot_paths(
        &ctx,
        GetHotPathsInput {
            limit: 10,
            min_fan_in: 1,
        },
    )
    .await
    .expect("the MCP answers with the hot paths");
    assert!(!hot.hot_paths.is_empty(), "the fixture declares a hot path");
    for entry in &hot.hot_paths {
        let Some(declared) = declared_column(&entry.symbol) else {
            continue;
        };
        assert_eq!(
            entry.column, declared,
            "the hot path of {} publishes a column that is not the column the file declares",
            entry.symbol
        );
    }

    // The call hierarchy says where each caller lives.
    let hierarchy = handle_get_call_hierarchy(
        &ctx,
        GetCallHierarchyInput {
            symbol_name: "beta".to_string(),
            direction: CallDirection::Incoming,
            depth: 1,
            include_external: false,
            compressed: false,
        },
    )
    .await
    .expect("the MCP answers with the call hierarchy");
    assert!(!hierarchy.calls.is_empty(), "the fixture declares a caller");
    for call in &hierarchy.calls {
        let Some(declared) = declared_column(&call.symbol) else {
            continue;
        };
        assert_eq!(
            call.column, declared,
            "the call to {} publishes a column that is not the column the file declares",
            call.symbol
        );
    }
}

/// The published column points at the declaration it names.
///
/// The column a tool publishes has to be the column of the character
/// the declaration starts at: that is what makes it usable. A column
/// that counts from zero points at the character before it, and a
/// consumer that carries that column to another tool lands on the
/// wrong character.
#[tokio::test]
async fn the_published_column_points_at_the_declaration_it_names() {
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

    let mut checked = 0;
    for entry in &answer.entry_points {
        let Some(line) = declared_line(&entry.name) else {
            continue;
        };
        assert_eq!(
            character_at(line, entry.location.column),
            Some('f'),
            "the published column of {} points at {:?}, not at the start of its declaration",
            entry.name,
            character_at(line, entry.location.column)
        );
        checked += 1;
    }
    assert!(checked > 0, "the fixture declares an entry point to check");
}

/// A published column comes back to the declaration it names.
///
/// This is the round trip: the graph publishes a position, and the
/// tool that takes a position reads the code at that position. The
/// column that comes back has to be the same declaration, and the
/// column before it has to be a different one — otherwise the base is
/// not the one the product says it is.
#[test]
fn a_published_column_comes_back_to_the_declaration_it_names() {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-core-column-round-trip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("the fixture directory");
    let path = dir.join("lib.rs");
    std::fs::write(&path, FIXTURE).expect("the fixture file");

    // The column of the declaration of `beta`, the way a person reads
    // it: the twenty-fourth character of the line.
    let published = declared_column("beta").expect("the fixture declares beta");

    let service = SymbolCodeService::new();
    let code = service
        .get_symbol_code(path.to_str().expect("a path"), 1, published)
        .expect("the code at the published column is read")
        .code;
    assert!(
        code.contains("fn beta"),
        "the published column points at beta: {code}"
    );

    // The column before it is the space in front of the declaration,
    // and it is not beta.
    let before = service
        .get_symbol_code(path.to_str().expect("a path"), 1, published - 1)
        .expect("the code at the column before is read")
        .code;
    assert!(
        !before.contains("fn beta"),
        "the column before the published one does not point at beta: {before}"
    );
}

/// The catalogue says which base the column is in.
///
/// A consumer that reads a column and carries it to another tool has
/// to know what it is counting. Every column the catalogue publishes
/// says so, and the capture of what the runtime publishes says the
/// same thing.
#[test]
fn the_catalogue_declares_the_base_of_every_column_it_publishes() {
    let catalogue: serde_json::Value =
        serde_json::from_slice(&std::fs::read(CATALOG).expect("the catalogue is readable"))
            .expect("the catalogue is JSON");
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(CAPTURE).expect("the capture is readable"))
            .expect("the capture is JSON");

    let mut declared = 0;
    for tool in catalogue["tools"].as_array().expect("the tools") {
        let name = tool["name"].as_str().expect("a tool has a name");
        let properties = tool["input_schema"]["properties"]
            .as_object()
            .expect("the tool declares its input properties");
        for (property, schema) in properties {
            if property != "column" && property != "col" {
                continue;
            }
            let description = schema["description"].as_str().unwrap_or_default();
            assert!(
                description.contains("1-indexed"),
                "the catalogue does not say which base the column of `{name}` is in: {description:?}"
            );
            declared += 1;
        }
    }
    assert!(
        declared > 0,
        "the catalogue publishes tools that take a column"
    );

    // The capture is what the runtime publishes: it has to say the
    // same thing about the same column.
    for tool in capture.as_array().expect("the captured tools") {
        let name = tool["name"].as_str().expect("a tool has a name");
        let properties = tool["inputSchema"]["properties"]
            .as_object()
            .expect("the tool declares its input properties");
        for (property, schema) in properties {
            if property != "column" && property != "col" {
                continue;
            }
            let description = schema["description"].as_str().unwrap_or_default();
            assert!(
                description.contains("1-indexed"),
                "the capture does not say which base the column of `{name}` is in: {description:?}"
            );
        }
    }
}

/// The catalogue does not declare a column in the base it retired.
///
/// A catalogue that says "0-indexed" somewhere next to a surface that
/// publishes one is a catalogue that lies about half the product.
#[test]
fn the_catalogue_retires_the_base_of_the_column_it_carried() {
    let catalogue: serde_json::Value =
        serde_json::from_slice(&std::fs::read(CATALOG).expect("the catalogue is readable"))
            .expect("the catalogue is JSON");

    for tool in catalogue["tools"].as_array().expect("the tools") {
        let name = tool["name"].as_str().expect("a tool has a name");
        let properties = tool["input_schema"]["properties"]
            .as_object()
            .expect("the tool declares its input properties");
        for (property, schema) in properties {
            if property != "column" && property != "col" {
                continue;
            }
            let description = schema["description"].as_str().unwrap_or_default();
            assert!(
                !description.contains("0-indexed"),
                "the catalogue still declares `{name}.{property}` in the retired base: {description:?}"
            );
        }
    }
}
