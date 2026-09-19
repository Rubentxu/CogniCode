# H4.4 — MCP Continuation Chain Real Server Verification

> Cycle: `h44-mcp-continuation-chain-real-server`
> Goal: prove that the MCP server delivers pages 2..n byte-exact over the
> real continuation chain (stdio transport). Closes the gap left by H4.3
> and H4.3.x: those closed the *reconstructor's* contract for the
> reconstructed content; H4.4 closes the *server's* contract for what the
> chain actually delivers.
>
> Spec: `specs/spec.md` (this change copies the global spec).

## Tasks

### T1 — Harness scaffold

- [ ] Add `crates/cognicode-mcp/tests/continuation_e2e.rs` with a helper
      that spawns `target/release/cognicode-mcp --cwd <repo>` over stdio
      and exposes a typed `McpClient::call_tool(name, args) -> Value`.
- [ ] Helper MUST perform `initialize` + `notifications/initialized`
      before any `tools/call`. Helper MUST tear down the child on drop.

### T2 — Chain follower

- [ ] Implement `follow_chain(initial_response) -> ChainResult` in the test
      file: accumulates pages until `has_more=false`, surfaces any
      JSON-RPC error or empty `content` mid-chain, returns the
      concatenated bytes and per-page SHA-256.
- [ ] The follower MUST NOT use the reconstructor's disk-mode. If a page
      reports `has_more=true` and `next_token=None`, the chain result is
      `Error` not `Complete`.

### T3 — Per-page invariant check

- [ ] After each page, compute SHA-256 of the accumulated bytes and
      compare against the SHA-256 of the file-on-disk prefix of the same
      length. Mismatch at any boundary → `ChainResult::BoundaryMismatch
      { page, accumulated_sha, expected_sha, offset }`.

### T4 — Scenario harness

- [ ] Implement the five Tier-1 read_source scenarios as a single
      `#[test]` (or one per scenario, marked `#[ignore]` if slow).
      Each test:
      - Spawns MCP server rooted at the right repo subpath
        (`sandbox/repos/anyhow`, `sandbox/repos/tokio/tokio`,
        `sandbox/repos/serde/serde`, `sandbox/repos/ripgrep/crates/cli`,
        `sandbox/repos/clap/clap_builder`).
      - Calls `read_file` with `mode=raw`.
      - Follows the chain.
      - Asserts SHA accumulated == SHA on disk.
      - Asserts per-page invariants.

### T5 — Latency + page-count capture

- [ ] Record `pages: u32`, `latency_p50/p95_ms: u64` per scenario and
      emit a one-line summary per scenario. Latency capture is
      informational; it does NOT gate GREEN.

### T6 — Wire the test into `cargo test`

- [ ] Ensure `cargo test -p cognicode-mcp --test continuation_e2e` runs
      the harness. If the test is `#[ignore]` (slow / spawns child),
      document why and provide a non-ignored smoke version that runs in
      <2s on a single scenario.

### T7 — Closeout

- [ ] Re-run `cargo test -p cognicode-mcp --test continuation_e2e` from
      a fresh shell to confirm reproducibility.
- [ ] Re-run `cargo test -p cognicode-core --lib` to confirm the
      reconstructor's existing tests still PASS (regression).
- [ ] Run `cargo fmt --check` and `cargo clippy --no-deps -- -D
      warnings` on the touched crates.
- [ ] Commit as a single SHA `H4.4`. Conventional commit, no AI
      trailers, no streak increment, no v1.0.0, no auto-open of H4.5.
- [ ] Update `.agent/TESTING-STATE.md` with the H4.4 closeout block.
- [ ] Push fast-forward.

## Out of scope

- Disk-mode reconstructor (closed in H4.3.x).
- Sandbox orchestrator (unchanged).
- HTTP transport (out of scope; only stdio is exercised).
- `search_content` scenarios (no pagination).
- Stress / load benchmarks.
- v1.0.0 declaration.
- Streak increment.
- H4.5 opening.

## RED if any of

- Any of the five scenarios fails to deliver SHA-256 == disk SHA.
- Any continuation chain errors mid-run on the canonical corpus.
- A scenario that should be single-page issues a continuation.
- A scenario that should be multi-page terminates after one page.
- The harness silently swallows an error and reports PASS.
