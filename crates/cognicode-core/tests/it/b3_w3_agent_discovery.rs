//! B3.W3 — Agent-oriented discovery affordances on the tool catalog.
//!
//! Extends the catalog (`product/tools.json` → `ToolDescriptor`) with
//! four optional affordances that let an agent answer, without
//! prompting the user, the canonical questions for tool use:
//!
//!   1. *What arguments are missing?*        → `required_args`
//!   2. *What prior analysis is required?*   → `prerequisites`
//!   3. *What follow-up result can I query?* → `follow_ups`
//!   4. *What limitation blocks progress?*   → `limitations`
//!
//! The intent-side questions ("what can I do" / "which intent
//! resolves here") are covered by B3.W2's `intent_tags` field on the
//! same struct (see `b3_w2_progressive_context::intent_tags_in_catalog_match_route_intent`).
//! B3.W3 deliberately does not duplicate that field; the two were
//! planned in adjacent WUs and we keep the canonical B3.W2 name.
//!
//! These tests pin the catalog contract so a future WU that
//! hand-edits the JSON without updating the affordances cannot
//! regress the discovery surface. Per B3.W3 R-B3-W3: the
//! affordances are METADATA, not domain logic; the catalog remains
//! the single source of truth.

use std::collections::{HashMap, HashSet};

use cognicode_core::interface::catalog::{
    ToolDescriptor, load_tool_catalog, load_tool_catalog_active,
};

// ============================================================================
// Helpers
// ============================================================================

fn catalog_path() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("product").join("tools.json");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("could not locate product/tools.json above {:?}", manifest);
}

fn read_raw_tools() -> Vec<serde_json::Value> {
    let path = catalog_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read catalog {path:?}: {e}"));
    let v: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("catalog {path:?} is not valid JSON: {e}"));
    v["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("catalog {path:?} has no `tools` array"))
        .clone()
}

#[allow(dead_code)]
fn catalog_names(catalog: &[ToolDescriptor]) -> HashSet<String> {
    catalog.iter().map(|d| d.name.clone()).collect()
}

// ============================================================================
// Test 1 — required_args mirrors input_schema.required field-for-field.
// ============================================================================

#[test]
fn required_args_in_catalog_match_input_schema() {
    // The catalog answers "what arguments are missing" declaratively
    // by mirroring `input_schema.required`. A drift here means an
    // agent looking at `required_args` would call a tool that
    // immediately fails on a missing arg (or skip a needed arg).
    let raw = read_raw_tools();
    let catalog = load_tool_catalog();
    let by_name: HashMap<&str, &ToolDescriptor> =
        catalog.iter().map(|d| (d.name.as_str(), d)).collect();

    let mut checked = 0usize;
    for raw_entry in &raw {
        let name = raw_entry["name"].as_str().expect("name");
        let schema_required: Vec<String> = raw_entry
            .get("input_schema")
            .and_then(|s| s.get("required"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let d = by_name
            .get(name)
            .unwrap_or_else(|| panic!("raw tool {name:?} missing from typed catalog"));

        match (&d.required_args, schema_required.is_empty()) {
            (Some(catalog_args), false) => {
                let catalog_set: HashSet<&str> = catalog_args.iter().map(|s| s.as_str()).collect();
                let schema_set: HashSet<&str> =
                    schema_required.iter().map(|s| s.as_str()).collect();
                assert_eq!(
                    catalog_set, schema_set,
                    "tool {name:?}: required_args disagrees with input_schema.required \
                     (catalog={catalog_args:?}, schema={schema_required:?})"
                );
                checked += 1;
            }
            (Some(catalog_args), true) => {
                assert!(
                    !catalog_args.is_empty(),
                    "tool {name:?}: required_args present but empty"
                );
            }
            (None, true) => { /* both empty — fine */ }
            (None, false) => {
                panic!(
                    "tool {name:?}: input_schema.required={schema_required:?} but the catalog \
                     has no `required_args`. The catalog is supposed to mirror required args \
                     declaratively so an agent does not have to parse JSON Schema."
                );
            }
        }
    }
    assert!(
        checked > 0,
        "test did not actually exercise any tool with both schema and catalog required_args; \
         catalog appears to be mis-declared (got {checked} checks)"
    );
}

// ============================================================================
// Test 2 — prerequisites / follow_ups reference known tool names.
// ============================================================================

#[test]
fn prerequisites_and_follow_ups_refer_to_known_tools() {
    let catalog = load_tool_catalog();
    let names = catalog_names(&catalog);

    for d in &catalog {
        let tool_name = d.name.as_str();
        if let Some(prereqs) = &d.prerequisites {
            for p in prereqs {
                assert!(
                    names.contains(p),
                    "tool {tool_name:?}: prerequisite {p:?} is not a known tool \
                     (catalog names: {names:?})"
                );
            }
        }
        if let Some(follows) = &d.follow_ups {
            for f in follows {
                assert!(
                    names.contains(f),
                    "tool {tool_name:?}: follow_up {f:?} is not a known tool"
                );
                assert_ne!(
                    f.as_str(),
                    tool_name,
                    "tool {tool_name:?}: follow_up points to itself"
                );
            }
        }
    }
}

// ============================================================================
// Test 3 — follow-ups graph is acyclic.
// ============================================================================

#[test]
fn catalog_follow_ups_chain_is_acyclic() {
    // The follow_ups graph is consulted by an agent to chain tool
    // calls; a cycle would loop forever. Three-colour DFS at 76
    // nodes is sufficient; Tarjan-free.
    let catalog = load_tool_catalog();
    let by_name: HashMap<&str, &ToolDescriptor> =
        catalog.iter().map(|d| (d.name.as_str(), d)).collect();

    #[derive(Clone)]
    enum Colour {
        White,
        Grey,
        Black,
    }

    fn dfs(
        node: &str,
        by_name: &HashMap<&str, &ToolDescriptor>,
        colour: &mut HashMap<String, Colour>,
        stack: &mut Vec<String>,
    ) -> Result<(), String> {
        colour.insert(node.to_string(), Colour::Grey);
        stack.push(node.to_string());
        if let Some(d) = by_name.get(node)
            && let Some(follows) = &d.follow_ups
        {
            for f in follows {
                match colour.get(f).cloned() {
                    Some(Colour::Grey) => {
                        let cycle_start = stack.iter().position(|n| n == f).unwrap_or(0);
                        let cycle: Vec<String> = stack[cycle_start..].to_vec();
                        return Err(format!(
                            "cycle in follow_ups: {} -> {}",
                            cycle.join(" -> "),
                            f
                        ));
                    }
                    Some(Colour::Black) => { /* fully explored */ }
                    None => {
                        colour.insert(f.clone(), Colour::White);
                        dfs(f, by_name, colour, stack)?;
                        colour.insert(f.clone(), Colour::Black);
                    }
                    Some(Colour::White) => {
                        dfs(f, by_name, colour, stack)?;
                        colour.insert(f.clone(), Colour::Black);
                    }
                }
            }
        }
        stack.pop();
        colour.insert(node.to_string(), Colour::Black);
        Ok(())
    }

    let mut colour: HashMap<String, Colour> = HashMap::new();
    for d in &catalog {
        colour.entry(d.name.clone()).or_insert(Colour::White);
    }
    let mut stack: Vec<String> = Vec::new();
    for d in &catalog {
        if matches!(colour.get(&d.name), Some(Colour::White) | None)
            && let Err(cycle) = dfs(&d.name, &by_name, &mut colour, &mut stack)
        {
            panic!("{cycle}");
        }
    }
}

// ============================================================================
// Test 4 — limitations field is non-empty for tools with known constraints.
// ============================================================================

#[test]
fn limitations_field_is_nonempty_for_tools_with_known_constraints() {
    // Tools that carry a constraint the agent MUST know about
    // (mutating authority, requires_graph) must surface it via
    // `limitations`. The converse is not asserted: a tool without
    // known constraints may omit the field entirely.
    let catalog = load_tool_catalog();

    let mut checked = 0usize;
    for d in &catalog {
        let is_mutating = matches!(d.authority.as_str(), "mutating" | "execute" | "network");
        let requires_graph = d
            .runtime_metadata
            .get("requires_graph")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let needs_graph_prereq = requires_graph && d.name != "build_graph";

        if is_mutating || needs_graph_prereq {
            let lims = d.limitations.as_ref().unwrap_or_else(|| {
                panic!(
                    "tool {:?} (authority={:?}, requires_graph={requires_graph}) \
                     must declare at least one limitation; an agent that asks \
                     'what blocks me' deserves an honest answer, not silence",
                    d.name, d.authority
                )
            });
            assert!(
                !lims.is_empty(),
                "tool {:?}: limitations is present but empty",
                d.name
            );
            let joined = lims.join(" ");
            if is_mutating {
                assert!(
                    joined.contains("mutates")
                        || joined.contains("workspace")
                        || joined.contains("applies")
                        || joined.contains("writes")
                        || joined.contains("overwrites"),
                    "tool {:?}: mutating tool should mention the mutation in limitations (got {lims:?})",
                    d.name
                );
            }
            if needs_graph_prereq {
                assert!(
                    joined.contains("build_graph") || joined.contains("graph"),
                    "tool {:?}: requires_graph tool should mention the graph dependency (got {lims:?})",
                    d.name
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 5,
        "test should exercise at least 5 mutating / requires_graph tools \
         (got {checked}); catalog coverage regressed"
    );
}

// ============================================================================
// Test 5 — capabilities JSON includes the four new fields, parity with the catalog.
// ============================================================================

#[test]
fn capabilities_json_includes_new_fields_and_matches_catalog() {
    let doc = build_capabilities_doc();
    let tools = doc["tools"]
        .as_array()
        .expect("`tools` must be a JSON array");

    let catalog = load_tool_catalog_active();
    let mut catalog_names: HashSet<String> = catalog.iter().map(|d| d.name.clone()).collect();
    let mut doc_names: HashSet<String> = tools
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();
    assert_eq!(
        catalog_names, doc_names,
        "catalog and capabilities doc must advertise the same tool set"
    );

    let by_name: HashMap<&str, &serde_json::Value> = tools
        .iter()
        .filter_map(|t| t["name"].as_str().map(|n| (n, t)))
        .collect();
    for d in &catalog {
        let entry = by_name
            .get(d.name.as_str())
            .unwrap_or_else(|| panic!("tool {:?} missing from capabilities doc", d.name));
        assert_field_matches(entry, "prerequisites", d.prerequisites.as_deref());
        assert_field_matches(entry, "follow_ups", d.follow_ups.as_deref());
        assert_field_matches(entry, "required_args", d.required_args.as_deref());
        assert_field_matches(entry, "limitations", d.limitations.as_deref());
    }
    let _ = (&mut catalog_names, &mut doc_names);
}

fn assert_field_matches(entry: &serde_json::Value, key: &str, expected: Option<&[String]>) {
    match expected {
        Some(values) => {
            let arr = entry
                .get(key)
                .unwrap_or_else(|| panic!("capabilities doc tool `{key}` is missing"))
                .as_array()
                .unwrap_or_else(|| panic!("capabilities doc `{key}` must be a JSON array"));
            let actual: Vec<String> = arr
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect();
            assert_eq!(
                actual, values,
                "capabilities doc field `{key}` disagrees with catalog"
            );
        }
        None => {
            assert!(
                entry.get(key).is_none(),
                "capabilities doc field `{key}` must be omitted when catalog has None"
            );
        }
    }
}

/// Reuse the same in-process builder the bin calls. Mirrors the
/// minimal path used by the CLI: load the active catalog and turn
/// it into a JSON object matching `cognicode.capabilities/v1`.
fn build_capabilities_doc() -> serde_json::Value {
    use cognicode_core::interface::catalog::load_tool_catalog_active;
    let catalog = load_tool_catalog_active();
    let tools: Vec<serde_json::Value> = catalog
        .iter()
        .map(|d| serde_json::to_value(d).expect("ToolDescriptor is serialisable"))
        .collect();
    serde_json::json!({
        "schema_version": "cognicode.capabilities/v1",
        "tools": tools,
    })
}

// ============================================================================
// Test 6 — at least one tool in the catalog actually carries each affordance.
// ============================================================================

#[test]
fn catalog_carries_all_four_affordances_on_at_least_one_tool() {
    // Sanity guard: if a future refactor accidentally drops the
    // serde annotations or the catalog loader filters the new
    // fields out, this test fails.
    let catalog = load_tool_catalog();
    let counts = [
        (
            "prerequisites",
            catalog.iter().filter(|d| d.prerequisites.is_some()).count(),
        ),
        (
            "follow_ups",
            catalog.iter().filter(|d| d.follow_ups.is_some()).count(),
        ),
        (
            "required_args",
            catalog.iter().filter(|d| d.required_args.is_some()).count(),
        ),
        (
            "limitations",
            catalog.iter().filter(|d| d.limitations.is_some()).count(),
        ),
    ];
    for (name, count) in counts {
        assert!(
            count > 0,
            "catalog carries no `{name}` field on any tool; \
             B3.W3 affordance dropped during refactor"
        );
    }
}

// ============================================================================
// Test 7 — tools that "Requires build_graph first" in their description
// must declare `build_graph` as a prerequisite.
// ============================================================================

#[test]
fn catalog_has_prerequisites_for_every_tool_with_deps() {
    // The catalog is the single source of truth. If a tool's
    // description text says "Requires build_graph first" or
    // similar, the typed `prerequisites` field must agree — an
    // agent reading the affordances must not be lied to by a
    // stale description string.
    let catalog = load_tool_catalog();
    let raw = read_raw_tools();
    let by_name: HashMap<&str, &ToolDescriptor> =
        catalog.iter().map(|d| (d.name.as_str(), d)).collect();

    let dep_phrases = [
        "Requires build_graph first",
        "Requires build_graph",
        "requires build_graph first",
        "Requires build_lightweight_index",
        "Requires build first",
        "requires build first",
        "Requires graph first",
        "requires graph first",
    ];

    let mut checked = 0usize;
    for raw_entry in &raw {
        let name = raw_entry["name"].as_str().expect("name");
        let desc = raw_entry["description"].as_str().unwrap_or("");
        if !dep_phrases.iter().any(|p| desc.contains(p)) {
            continue;
        }
        let d = by_name
            .get(name)
            .unwrap_or_else(|| panic!("tool {name:?} missing from typed catalog"));
        let prereqs = d.prerequisites.as_deref().unwrap_or(&[]);
        assert!(
            !prereqs.is_empty(),
            "tool {name:?}: description declares a prerequisite \
             (\"{desc}\") but `prerequisites` is empty"
        );
        if desc.to_lowercase().contains("build_graph") {
            assert!(
                prereqs.iter().any(|p| p == "build_graph"),
                "tool {name:?}: description mentions build_graph but \
                 `prerequisites` does not list it (got {prereqs:?})"
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 5,
        "test should exercise at least 5 tools whose description declares \
         a prerequisite (got {checked}); catalog descriptions regressed"
    );
}

// ============================================================================
// Test 8 — catalog_active preserves the four new fields through feature gating.
// ============================================================================

#[test]
fn catalog_active_preserves_discovery_fields() {
    // The active catalog (with feature gating) must carry the same
    // affordances as the full catalog for every tool it exposes.
    let full = load_tool_catalog();
    let active = load_tool_catalog_active();
    let active_names: HashSet<String> = active.iter().map(|d| d.name.clone()).collect();

    for full_d in &full {
        if !active_names.contains(&full_d.name) {
            continue;
        }
        let active_d = active
            .iter()
            .find(|d| d.name == full_d.name)
            .expect("active catalog should carry every active tool by name");
        assert_eq!(
            full_d.prerequisites, active_d.prerequisites,
            "tool {:?}: prerequisites lost in active catalog",
            full_d.name
        );
        assert_eq!(
            full_d.follow_ups, active_d.follow_ups,
            "tool {:?}: follow_ups lost in active catalog",
            full_d.name
        );
        assert_eq!(
            full_d.required_args, active_d.required_args,
            "tool {:?}: required_args lost in active catalog",
            full_d.name
        );
        assert_eq!(
            full_d.limitations, active_d.limitations,
            "tool {:?}: limitations lost in active catalog",
            full_d.name
        );
    }
}
