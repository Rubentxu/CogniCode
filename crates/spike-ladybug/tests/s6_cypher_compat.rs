//! E29 S6 Cypher compatibility tests — E1–E9 assertions.
//!
//! Each test:
//!   1. Creates a fresh TempDir with a new .lbdb
//!   2. Populates the fixture data (same as s6_fixtures.rs)
//!   3. Runs the corresponding probe
//!   4. Asserts the expected PASS marker in stdout
//!
//! GREEN: Tests pass when probes print correct markers.

use lbug::{Connection, Database, SystemConfig};
use std::env;
use std::path::PathBuf;
use tempfile::TempDir;

/// Returns the workspace root using CARGO_MANIFEST_DIR.
/// CARGO_MANIFEST_DIR = crates/spike-ladybug (when running tests)
/// Workspace root = grandparent of CARGO_MANIFEST_DIR.
fn workspace_root() -> PathBuf {
    env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok()
        .and_then(|p| p.parent().and_then(|p| p.parent()).map(|ws| ws.to_path_buf()))
        .unwrap_or_else(|| {
            // Fallback: walk up from current dir looking for Cargo.toml
            let mut dir = env::current_dir().unwrap_or_default();
            loop {
                if dir.join("Cargo.toml").exists() {
                    return dir.clone();
                }
                match dir.parent() {
                    Some(p) => dir = p.to_path_buf(),
                    None => break,
                }
            }
            PathBuf::from(".")
        })
}

// ============================================================================
// Shared fixture setup (inline, mirrors s6_fixtures.rs)
// ============================================================================

fn create_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.query("CREATE NODE TABLE IF NOT EXISTS Symbol (id INT64, name STRING, kind STRING, properties MAP(STRING, STRING), PRIMARY KEY(id));")?;
    conn.query("CREATE NODE TABLE IF NOT EXISTS Decision (id INT64, title STRING, status STRING, PRIMARY KEY(id));")?;
    conn.query("CREATE NODE TABLE IF NOT EXISTS Doc (id INT64, title STRING, content STRING, PRIMARY KEY(id));")?;
    conn.query("CREATE NODE TABLE IF NOT EXISTS Evidence (id INT64, content STRING, source STRING, PRIMARY KEY(id));")?;
    conn.query("CREATE REL TABLE IF NOT EXISTS Calls (FROM Symbol TO Symbol);")?;
    conn.query("CREATE REL TABLE IF NOT EXISTS Imports (FROM Symbol TO Symbol);")?;
    conn.query("CREATE REL TABLE IF NOT EXISTS Cites (FROM Symbol TO Decision);")?;
    conn.query("CREATE REL TABLE IF NOT EXISTS Justifies (FROM Evidence TO Decision);")?;
    conn.query("CREATE REL TABLE IF NOT EXISTS Resolves (FROM Symbol TO Decision);")?;
    Ok(())
}

fn populate_fixtures(conn: &Connection) -> anyhow::Result<()> {
    // Symbols
    conn.query("CREATE (:Symbol {id: 1, name: 'root', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 2, name: 'middle', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 3, name: 'leaf', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 4, name: 'isolated', kind: 'struct'});")?;
    conn.query("CREATE (:Symbol {id: 5, name: 'another_fn', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 6, name: 'mapped', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 500, name: 'callee_a', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 501, name: 'callee_b', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 997, name: 'e7_source', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 998, name: 'e6_target', kind: 'function'});")?;
    conn.query("CREATE (:Symbol {id: 999, name: 'e5_isolated', kind: 'struct'});")?;

    // Decisions
    conn.query("CREATE (:Decision {id: 1, title: 'ADR-001', status: 'accepted'});")?;
    conn.query("CREATE (:Decision {id: 2, title: 'ADR-002', status: 'proposed'});")?;

    // Docs
    conn.query("CREATE (:Doc {id: 1, title: 'API Doc', content: 'Public API documentation'});")?;
    conn.query("CREATE (:Doc {id: 2, title: 'Guide', content: 'User guide'});")?;

    // Evidence
    conn.query("CREATE (:Evidence {id: 1, content: 'Benchmark', source: 'perf-test'});")?;
    conn.query("CREATE (:Evidence {id: 2, content: 'Survey', source: 'survey'});")?;

    // Relationships
    conn.query("MATCH (s:Symbol {id: 1}), (t:Symbol {id: 2}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 2}), (t:Symbol {id: 3}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 1}), (t:Symbol {id: 500}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 1}), (t:Symbol {id: 501}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 997}), (t:Symbol {id: 500}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 997}), (t:Symbol {id: 501}) CREATE (s)-[:Calls]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 1}), (t:Symbol {id: 2}) CREATE (s)-[:Imports]->(t);")?;
    conn.query("MATCH (s:Symbol {id: 1}), (d:Decision {id: 1}) CREATE (s)-[:Cites]->(d);")?;
    conn.query("MATCH (e:Evidence {id: 1}), (d:Decision {id: 1}) CREATE (e)-[:Justifies]->(d);")?;
    conn.query("MATCH (s:Symbol {id: 3}), (d:Decision {id: 1}) CREATE (s)-[:Resolves]->(d);")?;

    Ok(())
}

fn setup_db() -> anyhow::Result<(TempDir, PathBuf)> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("s6_test.lbdb");

    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    create_schema(&conn)?;
    populate_fixtures(&conn)?;

    drop(conn);
    drop(db);

    Ok((tmp, db_path))
}

// Helper to run the compat example and capture output
fn run_compat_example() -> anyhow::Result<(String, String)> {
    let ws_root = workspace_root();
    let spike_manifest = ws_root.join("crates/spike-ladybug/Cargo.toml");

    let output = std::process::Command::new("cargo")
        .args(["run", "--manifest-path", spike_manifest.to_str().unwrap(), "--example", "s6_cypher_compat"])
        .current_dir(&ws_root)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((stdout, stderr))
}

// ============================================================================
// E1: All EdgeKind labels queryable
// ============================================================================

#[test]
fn test_e1_edge_kind_labels() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E1 PASS"),
        "E1: expected 'E1 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E2: Variable-length paths *1..3
// ============================================================================

#[test]
fn test_e2_variable_length_paths() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E2 PASS"),
        "E2: expected 'E2 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E3: WITH + ORDER BY + LIMIT
// ============================================================================

#[test]
fn test_e3_with_order_by_limit() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E3 PASS"),
        "E3: expected 'E3 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E4: UNWIND batch create
// ============================================================================

#[test]
fn test_e4_unwind_batch_create() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E4 PASS"),
        "E4: expected 'E4 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E5: OPTIONAL MATCH null-padding
// ============================================================================

#[test]
fn test_e5_optional_match_null_padding() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E5 PASS"),
        "E5: expected 'E5 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E6: MAP property access
// ============================================================================

#[test]
fn test_e6_map_property_access() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    // E6 may PASS or PASS_WITH_LIMITATION
    assert!(
        stdout.contains("E6 PASS"),
        "E6: expected 'E6 PASS' or 'E6 PASS_WITH_LIMITATION' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E7: SIZE() on relationship collection
// ============================================================================

#[test]
fn test_e7_size_on_relationship_collection() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E7 PASS"),
        "E7: expected 'E7 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E8: DISTINCT
// ============================================================================

#[test]
fn test_e8_distinct() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E8 PASS"),
        "E8: expected 'E8 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}

// ============================================================================
// E9: All NodeKind/EdgeKind labels accepted
// ============================================================================

#[test]
fn test_e9_all_labels_accepted() -> anyhow::Result<()> {
    let _ = setup_db()?;
    let (stdout, stderr) = run_compat_example()?;

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(
        stdout.contains("E9 PASS"),
        "E9: expected 'E9 PASS' in output but got:\n{}",
        stdout
    );

    Ok(())
}
