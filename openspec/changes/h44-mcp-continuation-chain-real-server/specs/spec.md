# MCP Continuation Chain — Real Server Verification

## Purpose

H4.1 introduced the line-paginated `read_file` continuation contract:
the first response carries `truncated=true, has_more=true, next_token=…`,
and a follow-up call with that token returns the next page. H4.2 hardened
the token binding (path + size limits). H4.3 added an opt-in reconstructor
that can follow the chain and score against the reconstructed content.
H4.3.x added adversarial coverage for the reconstructor's disk-fallback
path and a provenance marker (`source = McpChain | DiskFallback`) on the
artifact.

What is **not** yet proven: that the MCP server actually delivers pages
2..n byte-exact over that chain. The H4.3 Tier-1 results for anyhow and
tokio were obtained via `DiskFallback`, not via a verified continuation
chain.

This spec closes that gap. It defines an integration harness that drives
the **real** `cognicode-mcp` binary over its stdio JSON-RPC transport,
follows the continuation chain to its natural end, and asserts that the
accumulated SHA-256 matches the file on disk byte-exact. The harness is
additive — it does not modify the sandbox orchestrator, the reconstructor,
or the server.

## Scope

In scope:
- Spawning `cognicode-mcp --cwd <repo>` as a child process and driving it
  via stdio JSON-RPC (same transport the sandbox orchestrator uses).
- Exercising the continuation chain for the five Tier-1 read_source
  scenarios (`serde`, `ripgrep`, `anyhow`, `tokio`, `clap`).
- Asserting SHA-256 byte-equality of the accumulated response against the
  file on disk, and the absence of dropped or duplicated bytes.

Out of scope:
- Modifying `cognicode-mcp` itself.
- Modifying `cognicode-core::sandbox_core::read_source_reconstructor`.
- Modifying the sandbox orchestrator.
- `DiskFallback` path (already covered by H4.3.x tests; deliberately
  bypassed here so this spec measures the MCP chain only).
- `search_content` scenarios (do not paginate).
- Stress / load / latency benchmarks.
- v1.0.0 declaration, streak increment, H4.5 opening.

## Requirements

### Requirement: continuation harness drives the real MCP server over stdio

The harness MUST spawn `target/release/cognicode-mcp --cwd <repo>` as a
child process with `stdin` and `stdout` piped, perform the MCP
`initialize` handshake, and then issue `tools/call` requests directly
against that child. The harness MUST NOT use the sandbox orchestrator,
HTTP, or any disk-mode fallback — only the JSON-RPC over stdio transport
that the orchestrator itself uses.

#### Scenario: harness can initialize and shut down a child MCP server

- GIVEN a built `target/release/cognicode-mcp` binary exists
- AND a temp directory is set up as a fake workspace containing a small
  Rust source file
- WHEN the harness spawns the binary with `--cwd <tempdir>` and sends
  `initialize` followed by `tools/shutdown` (or kills the child on
  teardown)
- THEN the `initialize` response is received within 5 seconds
- AND the child exits cleanly

### Requirement: continuation chain accumulates byte-exact content

When `tools/call` returns a `read_file` response with
`truncated=true, has_more=true, next_token=Some(t)`, the harness MUST
issue a follow-up call with `arguments = { "continuation_token": t }`
and concatenate the `content` of each response in order. The harness
MUST stop when a response has `has_more=false`. The harness MUST detect
and report the following faults without silently succeeding:

- A `next_token` that returns fewer bytes than the previous page's
  `suggested_chunk_size` without signalling end-of-file.
- A repeated `next_token` (loop).
- An empty `content` field on a non-final page.
- A `read_file` error from the server mid-chain.

#### Scenario: anyhow src/lib.rs reconstructs byte-exact via the chain

- GIVEN the harness is connected to a child MCP server rooted at
  `sandbox/repos/anyhow`
- WHEN the harness calls `read_file` with `arguments = { path:
  "src/lib.rs", mode: "raw" }` and follows the `next_token` chain to
  its natural end
- THEN the harness issues at least one continuation call
- AND the concatenation of every page's `content[0].text` (decoded as a
  JSON object) yields the same bytes as `sandbox/repos/anyhow/src/lib.rs`
  on disk
- AND the SHA-256 of the concatenation equals the SHA-256 of the file on
  disk
- AND the harness records the number of pages and the per-page latency

#### Scenario: tokio src/lib.rs reconstructs byte-exact via the chain

- GIVEN the harness is connected to a child MCP server rooted at
  `sandbox/repos/tokio/tokio`
- WHEN the harness calls `read_file` with `arguments = { path:
  "src/lib.rs", mode: "raw" }` and follows the `next_token` chain to
  its natural end
- THEN the harness issues at least one continuation call
- AND the concatenation of every page's `content[0].text` equals the
  bytes of `sandbox/repos/tokio/tokio/src/lib.rs` on disk
- AND the SHA-256 of the concatenation equals the SHA-256 of the file on
  disk

#### Scenario: single-page scenarios terminate without continuations

- GIVEN the harness is connected to a child MCP server rooted at
  `sandbox/repos/serde` (or `ripgrep/crates/cli`, or
  `clap/clap_builder`)
- WHEN the harness calls `read_file` with the appropriate arguments and
  inspects the response
- THEN the response has `has_more=false` and no `next_token`
- AND the page content equals the bytes of the corresponding file on
  disk
- AND the harness records zero continuation calls for these scenarios

### Requirement: per-page invariant — no dropped, duplicated, or reordered bytes

For every page boundary in the chain, the harness MUST assert that the
concatenation of bytes `[0..k]` after page `k` equals the SHA-256 of the
prefix `[0..offset_k]` of the file on disk (where `offset_k` is the sum
of the bytes appended so far). If any boundary fails this check, the
harness MUST mark the chain as failing with a per-page diff.

#### Scenario: per-page SHA prefix is asserted at every continuation

- GIVEN a continuation chain with `N >= 2` pages
- WHEN the harness has accumulated pages `1..k` (for `k` in `2..=N`)
- THEN the SHA-256 of the accumulated bytes equals the SHA-256 of the
  first `accumulated_byte_count` bytes of the file on disk
- AND any mismatch is reported with both SHA-256 strings and the byte
  offset of the divergence

### Requirement: chain errors are surfaced, not swallowed

If any continuation call returns a JSON-RPC error, or returns a response
with `is_error=true`, or fails to parse, the harness MUST stop following
the chain and record the failure as a structured report (not a PASS).

#### Scenario: an MCP error mid-chain is reported, not retried

- GIVEN the harness is mid-chain and a continuation call returns a
  JSON-RPC error
- WHEN the harness records the result
- THEN the scenario is reported as `incomplete` with the error message
  AND no retry is attempted (per H4.2: continuation tokens are
  non-resumable across errors)

### Requirement: H4.4 GREEN criteria

H4.4 is GREEN if and only if all of the following hold simultaneously:

1. anyhow and tokio: SHA-256 of the reconstructed chain equals SHA-256
   of the file on disk, **byte-exact**, with at least one continuation
   call issued.
2. The three single-page scenarios (serde, ripgrep, clap) report
   `has_more=false` with no continuations issued and content matching the
   file on disk.
3. Per-page SHA prefixes match at every boundary.
4. Zero chain errors in the run.
5. Zero false positives: a scenario is reported PASS only when its
   reconstructed SHA matches the disk SHA byte-exact. SHA collision or
   partial reconstruction is NEVER accepted as PASS.

If any of the above fails, H4.4 is RED and the failure mode is documented
as the next-cycle debt (without auto-opening H4.5).

## Anti-requirements

The harness MUST NOT:
- Use the sandbox orchestrator.
- Use HTTP transport. Only stdio is in scope.
- Use `DiskFallback` from the reconstructor. The whole point of H4.4 is
  to test the chain directly; bypassing it defeats the purpose.
- Treat a SHA collision or partial match as PASS.
- Skip any of the five Tier-1 read_source scenarios.
- Declare v1.0.0 or increment the streak.
- Auto-open H4.5 on close.
