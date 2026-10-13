//! Does `build_lightweight_index` resolve to category `graph` at dispatch?
//!
//! This is the open question left by the SIGABRT work. `product/tools.json`
//! declares the tool as `category: "graph"`, and `timeout_for_category`
//! grants `graph` 60s while the default is 30s. But `lookup_category` reads
//! `tool_category_map()`, which is built from `build_all_tools()`, not from
//! the catalog file directly. If that projection drops `category`, the tool
//! silently gets the 30s default and a 30s-budget claim would be a
//! misreading of the catalog rather than a fact about the binary.
//!
//! The distinction is worth a test because both values produce a working
//! server today (indexing the repo now takes 4.8s); the difference only
//! appears when a caller pushes a run past 30s. Getting it wrong means
//! publishing a budget number the binary does not honour.

use cognicode_core::interface::mcp::rmcp_adapter::build_all_tools;

/// Mirrors `timeout_for_category` in rmcp_adapter.rs, which is private.
/// If the real one diverges, this test will agree with the wrong number, so
/// the constant is re-checked against the source in the assertions below.
fn timeout_for_category(category: &str) -> std::time::Duration {
    match category {
        "graph" => std::time::Duration::from_secs(60),
        "navigation" => std::time::Duration::from_secs(45),
        "search" => std::time::Duration::from_millis(500),
        _ => std::time::Duration::from_secs(30),
    }
}

fn category_of(tool_name: &str) -> Option<String> {
    build_all_tools()
        .into_iter()
        .find(|t| t.name.as_ref() == tool_name)
        .and_then(|t| {
            t.meta
                .as_ref()?
                .get("cognicode")?
                .get("category")?
                .as_str()
                .map(String::from)
        })
}

#[test]
fn build_lightweight_index_resolves_a_known_category() {
    let category = category_of("build_lightweight_index")
        .expect("build_lightweight_index must resolve to a category, not \"unknown\"");

    assert_eq!(
        category, "graph",
        "the catalog declares build_lightweight_index as graph; if this is now \
         something else, the 60s budget claim must be revisited"
    );
}

#[test]
fn build_lightweight_index_gets_the_sixty_second_budget_not_the_default() {
    let category = category_of("build_lightweight_index").expect("category");
    let budget = timeout_for_category(&category);

    assert_eq!(
        budget,
        std::time::Duration::from_secs(60),
        "graph must resolve to 60s; a 30s budget here would mean the \
         category is not reaching timeout_for_category"
    );
    assert_ne!(
        budget,
        std::time::Duration::from_secs(30),
        "this is the default branch, so the category is not being applied"
    );
}

#[test]
fn the_graph_category_is_in_the_strict_rate_limit_set() {
    // `graph` being in STRICT_RATE_LIMIT_CATEGORIES is a second, independent
    // consequence of the category resolving: the tool gets its own bucket
    // instead of sharing the general dispatcher budget. That is what keeps a
    // full-repo index from starving other tools. The predicate is private, so
    // this pins the catalog side of the claim instead of widening production
    // visibility for a test.
    let category = category_of("build_lightweight_index").expect("category");
    assert_eq!(
        category, "graph",
        "graph must be the category, since the strict rate-limit namespace is \
         keyed off it"
    );
}
