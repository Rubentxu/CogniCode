//! E29 S6 fixtures — populates a minimal LadybugDB for Cypher compatibility probing.
//!
//! Minimal DDL for S6 (matches S2 schema for EdgeKinds needed):
//!   Node tables: Symbol, Decision, Doc, Evidence
//!   Rel tables:  Calls, Imports, Cites, Justifies, Resolves (CorroboratedBy may be missing per D3)
//!
//! Fixture requirements per design.md D1:
//!   - E2: ≥1 Symbol with a 3-hop Calls chain
//!   - E5: ≥1 Symbol with 0 outgoing Calls
//!   - E7: ≥1 Symbol with exactly 2 outgoing Calls
//!   - E8: ≥2 Symbols sharing the same `kind`
//!   - E6: ≥1 Symbol with MAP `properties`

use lbug::{Connection, Database, SystemConfig};
use tempfile::TempDir;

fn create_schema(conn: &Connection) -> anyhow::Result<()> {
    // Minimal S6 schema — Symbol node
    conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Symbol (\
         id INT64, name STRING, kind STRING, properties MAP(STRING, STRING), PRIMARY KEY(id));"
    )?;

    // Decision node (for E9)
    conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Decision (\
         id INT64, title STRING, status STRING, PRIMARY KEY(id));"
    )?;

    // Doc node (for E9)
    conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Doc (\
         id INT64, title STRING, content STRING, PRIMARY KEY(id));"
    )?;

    // Evidence node (for E9)
    conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Evidence (\
         id INT64, content STRING, source STRING, PRIMARY KEY(id));"
    )?;

    // === Rel tables ===
    // Calls — used for E2 (variable-length paths), E7 (SIZE collection)
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Calls (\
         FROM Symbol TO Symbol);"
    )?;

    // Imports
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Imports (\
         FROM Symbol TO Symbol);"
    )?;

    // Cites (Symbol -> Decision)
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Cites (\
         FROM Symbol TO Decision);"
    )?;

    // Justifies (Evidence -> Decision)
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Justifies (\
         FROM Evidence TO Decision);"
    )?;

    // Resolves (Symbol -> Decision)
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Resolves (\
         FROM Symbol TO Decision);"
    )?;

    // Note: CorroboratedBy may not exist in S2 DDL (D3 schema gap)
    // We'll probe for it but don't assume it exists.

    Ok(())
}

fn populate_fixtures(conn: &Connection) -> anyhow::Result<(usize, usize)> {
    // === Symbols ===
    // Symbol 1: root of 3-hop Calls chain (E2), has 2 outgoing Calls (E7)
    conn.query(
        "CREATE (:Symbol {id: 1, name: 'root', kind: 'function'});"
    )?;
    // Symbol 2: middle hop
    conn.query(
        "CREATE (:Symbol {id: 2, name: 'middle', kind: 'function'});"
    )?;
    // Symbol 3: end of chain
    conn.query(
        "CREATE (:Symbol {id: 3, name: 'leaf', kind: 'function'});"
    )?;
    // Symbol 4: isolated (0 outgoing Calls — E5)
    conn.query(
        "CREATE (:Symbol {id: 4, name: 'isolated', kind: 'struct'});"
    )?;
    // Symbol 5: another function to share kind with Symbol 1 (E8 DISTINCT)
    conn.query(
        "CREATE (:Symbol {id: 5, name: 'another_fn', kind: 'function'});"
    )?;
    // Symbol 6: with MAP properties (E6)
    conn.query(
        "CREATE (:Symbol {id: 6, name: 'mapped', kind: 'function'});"
    )?;
    // Symbol 500, 501: targets for 2 Calls from Symbol 1 (E7)
    conn.query(
        "CREATE (:Symbol {id: 500, name: 'callee_a', kind: 'function'});"
    )?;
    conn.query(
        "CREATE (:Symbol {id: 501, name: 'callee_b', kind: 'function'});"
    )?;
    // Symbol 997: for E7 cleanup (2 outgoing Calls target)
    conn.query(
        "CREATE (:Symbol {id: 997, name: 'e7_source', kind: 'function'});"
    )?;
    // Symbol 998: for E6 MAP test
    conn.query(
        "CREATE (:Symbol {id: 998, name: 'e6_target', kind: 'function', properties: map(['codeowners'], ['team-alpha'])});"
    )?;
    // Symbol 999: for E5 null-padding test (isolated)
    conn.query(
        "CREATE (:Symbol {id: 999, name: 'e5_isolated', kind: 'struct'});"
    )?;
    // Symbols 100, 101: for E4 UNWIND batch create test
    conn.query(
        "CREATE (:Symbol {id: 100, name: 'unwind_a', kind: 'function'});"
    )?;
    conn.query(
        "CREATE (:Symbol {id: 101, name: 'unwind_b', kind: 'function'});"
    )?;

    // === Decision nodes (E9) ===
    conn.query(
        "CREATE (:Decision {id: 1, title: 'ADR-001', status: 'accepted'});"
    )?;
    conn.query(
        "CREATE (:Decision {id: 2, title: 'ADR-002', status: 'proposed'});"
    )?;

    // === Doc nodes (E9) ===
    conn.query(
        "CREATE (:Doc {id: 1, title: 'API Doc', content: 'Public API documentation'});"
    )?;
    conn.query(
        "CREATE (:Doc {id: 2, title: 'Guide', content: 'User guide'});"
    )?;

    // === Evidence nodes (E9) ===
    conn.query(
        "CREATE (:Evidence {id: 1, content: 'Benchmark shows 10x improvement', source: 'perf-test'});"
    )?;
    conn.query(
        "CREATE (:Evidence {id: 2, content: 'User survey confirms', source: 'survey'});"
    )?;

    // === Relationships ===
    // Calls chain: 1 -> 2 -> 3 (3 hops for E2)
    conn.query(
        "MATCH (s:Symbol {id: 1}), (t:Symbol {id: 2}) CREATE (s)-[:Calls]->(t);"
    )?;
    conn.query(
        "MATCH (s:Symbol {id: 2}), (t:Symbol {id: 3}) CREATE (s)-[:Calls]->(t);"
    )?;
    // Additional Calls for variable-length paths (1 -> 500, 1 -> 501 for E7)
    conn.query(
        "MATCH (s:Symbol {id: 1}), (t:Symbol {id: 500}) CREATE (s)-[:Calls]->(t);"
    )?;
    conn.query(
        "MATCH (s:Symbol {id: 1}), (t:Symbol {id: 501}) CREATE (s)-[:Calls]->(t);"
    )?;
    // E7: 997 -> 500, 997 -> 501 (2 outgoing Calls)
    conn.query(
        "MATCH (s:Symbol {id: 997}), (t:Symbol {id: 500}) CREATE (s)-[:Calls]->(t);"
    )?;
    conn.query(
        "MATCH (s:Symbol {id: 997}), (t:Symbol {id: 501}) CREATE (s)-[:Calls]->(t);"
    )?;

    // Imports (Symbol 1 imports Symbol 2)
    conn.query(
        "MATCH (s:Symbol {id: 1}), (t:Symbol {id: 2}) CREATE (s)-[:Imports]->(t);"
    )?;

    // Cites (Symbol 1 cites Decision 1)
    conn.query(
        "MATCH (s:Symbol {id: 1}), (d:Decision {id: 1}) CREATE (s)-[:Cites]->(d);"
    )?;

    // Justifies (Evidence 1 justifies Decision 1)
    conn.query(
        "MATCH (e:Evidence {id: 1}), (d:Decision {id: 1}) CREATE (e)-[:Justifies]->(d);"
    )?;

    // Resolves (Symbol 3 resolves Decision 1)
    conn.query(
        "MATCH (s:Symbol {id: 3}), (d:Decision {id: 1}) CREATE (s)-[:Resolves]->(d);"
    )?;

    // Count nodes and edges
    let mut rows = conn.query("MATCH (n) RETURN count(n);")?;
    let node_count: usize = if let Some(row) = rows.next() {
        row[0].to_string().parse().unwrap_or(0)
    } else { 0 };

    let mut rows = conn.query("MATCH ()-[r]->() RETURN count(r);")?;
    let edge_count: usize = if let Some(row) = rows.next() {
        row[0].to_string().parse().unwrap_or(0)
    } else { 0 };

    Ok((node_count, edge_count))
}

fn main() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("s6_test.lbdb");
    let path_str = db_path.to_str().unwrap();

    println!("S6_FIXTURE_READY: creating DB at {}", path_str);

    let db = Database::new(path_str, SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    create_schema(&conn)?;
    let (nodes, edges) = populate_fixtures(&conn)?;

    println!("S6_FIXTURE_POPULATED: {} nodes + {} edges", nodes, edges);
    println!("S6_FIXTURE_SCHEMA: Symbol, Decision, Doc, Evidence + Calls, Imports, Cites, Justifies, Resolves");

    // Drop connection to allow clean reopen
    drop(conn);
    drop(db);

    println!("S6_FIXTURE_DONE");
    Ok(())
}
