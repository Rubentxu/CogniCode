//! H2.2 (delivery-first-radical, package integration) — affordances
//! derived from the existing catalog, typed preconditions and
//! permissions; a read never offers a mutating affordance without
//! explicit capability.
//!
//! H2.2 spec: "Affordances derivadas del catálogo existente,
//! precondiciones y permisos tipados; una lectura nunca ofrece
//! acción mutante sin capacidad explícita."
//!
//! The catalog already encodes the three pieces: `authority`
//! (typed enum: `read` or `mutating`), `prerequisites` (typed
//! list of tool names), and `follow_ups` (typed list of tool
//! names — these are the affordances). The runtime_metadata
//! block carries a redundant `authority` and a `mutates_workspace`
//! boolean, which is the typed capability flag. This test pins
//! the contract:
//!
//! 1. `authority` is exactly one of `read` | `mutating`. A third
//!    value (e.g. `"unstable"` or `"delegated"`) would be a
//!    permissions regression.
//! 2. A `read` tool never carries `mutates_workspace = true` in
//!    runtime_metadata. Belt and suspenders for the typed
//!    permission.
//! 3. `prerequisites` and `follow_ups` are typed (every entry is
//!    a tool name that exists in the catalog). A dangling pointer
//!    would break the consumer's `next_action` chain silently.
//! 4. CORE H2.2 INVARIANT: a read tool's `follow_ups` never
//!    reference a mutating tool. The current surface satisfies
//!    this perfectly; the test pins the rule so any future
//!    contributor who adds a mutating follow-up to a read tool
//!    breaks the test on purpose and has to introduce an
//!    explicit capability declaration.
//! 5. Belt and suspenders: `runtime_metadata.authority` agrees
//!    with top-level `authority`. A drift between the two is a
//!    catalog desync that the MCP server would surface as
//!    contradictory metadata.
//!
//! Like H2.1, this is a read-only characterization. No production
//! code change. The catalog is the contract; the test is the
//! witness.

use serde_json::Value;
use std::collections::BTreeMap;

const CATALOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../product/tools.json");

fn load() -> Value {
    let bytes = std::fs::read(CATALOG).expect("read catalog");
    serde_json::from_slice::<Value>(&bytes).expect("parse catalog")
}

fn index<'a>(catalog: &'a Value) -> BTreeMap<&'a str, &'a Value> {
    catalog
        .get("tools")
        .and_then(Value::as_array)
        .expect("catalog has tools[]")
        .iter()
        .map(|t| {
            (
                t.get("name")
                    .and_then(Value::as_str)
                    .expect("tool has name"),
                t,
            )
        })
        .collect()
}

#[test]
fn authority_is_typed_as_read_or_mutating_only() {
    // The typed permission enum. A third value (e.g. "delegated",
    // "internal", "admin") would be a permissions regression;
    // consumers dispatch on `authority` and would silently treat
    // the new value as read, which is a security defect.
    let catalog = load();
    let tools = index(&catalog);
    let mut bad = Vec::new();
    for (name, t) in &tools {
        let auth = match t.get("authority").and_then(Value::as_str) {
            Some("read") | Some("mutating") => continue,
            other => other.unwrap_or("<missing>").to_string(),
        };
        bad.push((name.to_string(), auth));
    }
    assert!(
        bad.is_empty(),
        "every tool's authority must be exactly `read` or `mutating`; off-enum values: {bad:?}"
    );
    // Sanity: the catalog has both kinds, so the assertion
    // above is non-vacuous.
    let reads = tools
        .values()
        .filter(|t| t.get("authority").and_then(Value::as_str) == Some("read"))
        .count();
    let muts = tools
        .values()
        .filter(|t| t.get("authority").and_then(Value::as_str) == Some("mutating"))
        .count();
    assert!(reads > 0 && muts > 0, "the catalog must contain both kinds (else the audit is vacuous); got {reads} read, {muts} mutating");
}

#[test]
fn read_tools_never_carry_mutates_workspace_true() {
    // The typed capability flag. A read tool declaring
    // mutates_workspace=true is a contradiction that an
    // authz-aware consumer would have to reject — the audit
    // rejects it at catalog level instead.
    let catalog = load();
    let tools = index(&catalog);
    let mut bad = Vec::new();
    for (name, t) in &tools {
        if t.get("authority").and_then(Value::as_str) == Some("read") {
            let rm = t.get("runtime_metadata");
            let mutates = rm
                .and_then(|m| m.get("mutates_workspace"))
                .and_then(Value::as_bool);
            if mutates == Some(true) {
                bad.push(name.to_string());
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a read tool must never declare mutates_workspace=true; the typed capability contradicts authority: {bad:?}"
    );
}

#[test]
fn runtime_metadata_authority_agrees_with_top_level_authority() {
    // The catalog carries authority in two places. A drift
    // would be a catalog desync: tools/list would say one
    // thing and the runtime envelope would say another.
    let catalog = load();
    let tools = index(&catalog);
    let mut bad = Vec::new();
    for (name, t) in &tools {
        let top = t.get("authority").and_then(Value::as_str);
        let rm = t
            .get("runtime_metadata")
            .and_then(|m| m.get("authority"))
            .and_then(Value::as_str);
        if top != rm {
            bad.push((
                name.to_string(),
                top.unwrap_or("<missing>").to_string(),
                rm.unwrap_or("<missing>").to_string(),
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "runtime_metadata.authority must equal top-level authority; drifts: {bad:?}"
    );
}

#[test]
fn prerequisites_resolve_to_known_tools() {
    // Typed preconditions: every entry in `prerequisites` is a
    // tool name that exists in the catalog. A dangling pointer
    // would break the consumer's "I can call this only if X has
    // run" chain silently.
    let catalog = load();
    let tools = index(&catalog);
    let mut unresolved = Vec::new();
    for (name, t) in &tools {
        if let Some(arr) = t.get("prerequisites").and_then(Value::as_array) {
            for p in arr {
                let pname = p
                    .as_str()
                    .expect("prerequisite must be a string, not a structured value");
                if !tools.contains_key(pname) {
                    unresolved.push((name.to_string(), pname.to_string()));
                }
            }
        }
    }
    assert!(
        unresolved.is_empty(),
        "every prerequisite must resolve to a known tool; unresolved: {unresolved:?}"
    );
}

#[test]
fn follow_ups_resolve_to_known_tools() {
    // Typed affordances: every entry in `follow_ups` is a tool
    // name in the catalog. A dangling follow-up would be the
    // "suggested next action" that the consumer can't take.
    let catalog = load();
    let tools = index(&catalog);
    let mut unresolved = Vec::new();
    for (name, t) in &tools {
        if let Some(arr) = t.get("follow_ups").and_then(Value::as_array) {
            for f in arr {
                let fname = f
                    .as_str()
                    .expect("follow_up must be a string, not a structured value");
                if !tools.contains_key(fname) {
                    unresolved.push((name.to_string(), fname.to_string()));
                }
            }
        }
    }
    assert!(
        unresolved.is_empty(),
        "every follow_up (affordance) must resolve to a known tool; unresolved: {unresolved:?}"
    );
}

#[test]
fn read_tools_never_offer_mutating_follow_ups() {
    // CORE H2.2 INVARIANT. A read tool's affordance list must
    // point only at other read tools. Pointing a read tool at a
    // mutating tool would mean: "this read result, with no
    // further authorization, can mutate the workspace." That
    // violates the spec: "una lectura nunca ofrece acción
    // mutante sin capacidad explícita."
    //
    // The current catalog has no explicit-capability annotation.
    // Until one is introduced, the strict reading of the rule
    // is: read → {read}. A future contributor who wants a read
    // tool to surface a mutating follow-up must first add the
    // capability declaration (e.g. a per-tool
    // `affordances_requiring_capability: ["tool_name"]` list)
    // and update this test's exception. The contract change is
    // then visible in the diff.
    let catalog = load();
    let tools = index(&catalog);
    let mut violations = Vec::new();
    for (name, t) in &tools {
        if t.get("authority").and_then(Value::as_str) != Some("read") {
            continue;
        }
        if let Some(arr) = t.get("follow_ups").and_then(Value::as_array) {
            for f in arr {
                let fname = f.as_str().expect("string follow_up");
                if let Some(target) = tools.get(fname) {
                    if target.get("authority").and_then(Value::as_str) == Some("mutating") {
                        violations.push((name.to_string(), fname.to_string()));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "a read tool's follow_ups must point only at other read tools; \
         a read → mutating link is a permission defect that requires an \
         explicit capability declaration before it is allowed: {violations:?}"
    );
}

#[test]
fn requirements_block_is_self_consistent() {
    // The `requirements` block is the typed preconditions
    // summary: graph/persistence/cache/network. The
    // runtime_metadata block is the typed capability
    // summary. They must agree, because consumers read one
    // or the other and a drift would surface as
    // contradictory preconditions.
    let catalog = load();
    let tools = index(&catalog);
    let mut bad = Vec::new();
    for (name, t) in &tools {
        let r = t.get("requirements");
        let rm = t.get("runtime_metadata");
        let graph_req = r
            .and_then(|x| x.get("graph"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let graph_meta = rm
            .and_then(|x| x.get("requires_graph"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if graph_req != graph_meta {
            bad.push((name.to_string(), "graph".to_string(), graph_req, graph_meta));
        }
        let pers_req = r
            .and_then(|x| x.get("persistence"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let pers_meta = rm
            .and_then(|x| x.get("requires_persistence"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if pers_req != pers_meta {
            bad.push((name.to_string(), "persistence".to_string(), pers_req, pers_meta));
        }
    }
    assert!(
        bad.is_empty(),
        "requirements block must agree with runtime_metadata block; drifts: {bad:?}"
    );
}
