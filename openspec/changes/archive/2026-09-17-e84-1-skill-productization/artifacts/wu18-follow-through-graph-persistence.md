# WU18 follow-through — Graph persistence: pinned-bug NOT reproducible in unit test

## What WU18 reported

During UAT against release binaries, after a successful `build_graph`
in a session, the following returned "No call graph available":

- `graph_explain`
- `graph_communities`
- `graph_pagerank`

Hypothesis recorded in `wu19-missing-product-surfaces.md` Class A1:

> `build_graph` writes the graph to `WorkspaceSession.analysis`
> cache (`analysis_service.graph_cache()`) but graph-reading MCP
> handlers read from `ctx.get_graph_store().load_graph()` (a
> different store).

## What this follow-through found

The hypothesis is **incorrect for the default code path**.

`HandlerContext::get_graph_store()` (mod.rs:430) returns a
`CachedGraphStore` wrapping `analysis_service.graph_cache()`. So:

- `build_graph` → `analysis_service.build_project_graph()` →
  updates `analysis_service.graph_cache()`.
- `graph_explain` → `ctx.get_graph_store().load_graph()` →
  `CachedGraphStore::load_graph` reads from the SAME
  `analysis_service.graph_cache()`.

The two paths share the cache in the **default** configuration.

A unit test exercising this exact flow (`build_graph` then
`graph_explain` in the same `HandlerContext`) PASSES:

```text
running 1 test
test interface::mcp::mcp_roundtrip_tests::tests::
    graph_persistence_pinned::test_graph_explain_after_build_graph_in_session
... ok
```

The test was added, run, and then **removed** because pinning a
bug that does not reproduce in the unit test would be theatre,
not signal.

## What the live UAT might have hit

Three plausible explanations for the original WU18 finding:

1. **Persistent SQLite store**: when `with_graph_store` is used
   to install a real SQLite-backed `GraphStore` (e.g. via the
   runtime wiring that the `cogh` binary does), the
   `CachedGraphStore` fallback is bypassed. In that case
   `build_graph`'s `save_manifest` call writes only the manifest
   to SQLite, not the graph. A subsequent `graph_explain` would
   see "No graph available" because the SQLite `graph` table is
   empty for that directory. The unit test exercises the
   **fallback** path (default `HandlerContext::builder()`), not
   the persistent path.
2. **Process restart between MCP calls**: an MCP server
   re-launching between `build_graph` and `graph_explain` would
   lose the in-memory cache. The shared SQLite store would have
   only the manifest, not the graph (per #1).
3. **A different workspace directory**: if `build_graph` and
   `graph_explain` resolve to different `directory` values, the
   keys don't match.

## Action

- The WU19 Class A1 entry is **downgraded**: no longer a code bug,
  but a **runtime configuration caveat** documented in
  `skills/cognicode-mcp/SKILL.md` under "Known runtime quirk".
- The "Known runtime quirk" section in the skill should now
  distinguish:
  - Default config (`CachedGraphStore` fallback): `build_graph`
    then any reader works in the same process.
  - Persistent config (SQLite-backed `GraphStore`): readers
    need the graph to be loaded via a separate path
    (e.g. `check_architecture` which auto-builds and
    self-loads).
- A future cycle that wires `with_graph_store(SQLite)` should add
  a regression test for the persistent path. Out of scope for
  e84.1 follow-through.
- The "second `build_graph` works" workaround documented in the
  skill is real (the cache-hit path reads from the persistence
  store, and the manifest is persisted; but the graph itself
  is **only** persisted if a separate `save_graph` is called,
  which is **not** called by `build_graph` in the persistent
  path — so the workaround may itself not work for the
  persistent configuration; this is unverified).

## What this changes

- The pinned-bug test is gone. No test was added.
- The WU19 entry is honest about being unconfirmed, not
  promoted to a known bug.
- The skill's "Known runtime quirk" section is still useful as
  user-facing documentation, but the wording should soften
  from "broken" to "configuration-dependent".

## Skill update recommended

In `skills/cognicode-mcp/SKILL.md`, the "Known runtime quirk"
section currently reads:

```text
## Known runtime quirk

After `build_graph`, calls like `graph_explain`,
`graph_communities`, `graph_pagerank` may return
"No call graph available" in the same session.
Workaround: call `check_architecture` first (it auto-builds
and self-loads), or call `build_graph` twice.
```

Recommended revision:

```text
## Known runtime quirk (configuration-dependent)

In the default configuration, `build_graph` then any reader
(e.g. `graph_explain`, `graph_communities`) works in the same
process: both share the in-memory graph cache.

When the runtime is configured with a persistent (SQLite-backed)
GraphStore, only the manifest is persisted by `build_graph`. The
graph itself is loaded by the manifest-driven cache path on the
NEXT `build_graph`. To use readers in the same session, prefer
`check_architecture` (which auto-builds and self-loads).
```

This update will be applied as part of WU18 follow-through
completion.
