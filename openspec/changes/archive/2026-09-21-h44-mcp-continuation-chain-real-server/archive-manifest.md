# Archive Manifest — h44-mcp-continuation-chain-real-server

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/h44-mcp-continuation-chain-real-server` |
| Path | b-direct |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-with-receipt`** |
| Superseded at | 2026-09-21T06:34Z |

## Outcome

CLOSED. The cycle was scoped to prove that the MCP server delivers
pages 2..n byte-exact over the real continuation chain (stdio
transport), closing the server-side contract that H4.3 and H4.3.x
left for the *reconstructor's* disk-mode contract.

## Acceptance verdicts

| T# | Task | Status |
|---|---|---|
| T1 | Harness scaffold (`continuation_e2e.rs` + `McpClient`) | ✅ CLOSED |
| T2 | Chain follower (`follow_chain` + SHA-256 + JSON-RPC errors) | ✅ CLOSED |
| T3 | Per-page invariant (SHA-256 accumulated == file prefix) | ✅ CLOSED |
| T4 | Scenario harness (5 Tier-1 read_source scenarios) | ✅ CLOSED |
| T5 | Latency + page-count capture | ✅ CLOSED |
| T6 | Wire into `cargo test` | ✅ CLOSED |

**Verify verdict**: PASS — `cargo test -p cognicode-mcp --test
continuation_e2e` 5/5 PASS (per cycle receipt and per umbrella
roadmap-auto-discovery verification-report re-confirmation).

## Commits produced

| SHA | Subject |
|-----|---------|
| `91548c6c` | test(mcp): verify continuation chain delivers pages 2..n byte-exact over real server (H4.4) |

## Cross-references

- Spec: `openspec/changes/archive/2026-09-21-h44-mcp-continuation-chain-real-server/specs/spec.md`
- Tasks: `openspec/changes/archive/2026-09-21-h44-mcp-continuation-chain-real-server/tasks.md`
- H4.3 / H4.3.x baseline (reconstructor contract):
  - `d0e7646f` test(read-file): H4.3.x — adversarial coverage for disk-mode reconstruction
  - `3da5468e` feat(read-file): H4.3 — opt-in read_source_full continuation reconstruction
- H4.4 sibling (continuation token binding):
  - `fc281f81` fix(read-file): H4.2 — harden continuation token binding (path + bounds)

## Closure semantics

```text
implementation      CLOSED (real)
verification       GREEN  (real, 5/5 PASS)
archive closure    DONE   (this manifest)
```

The H4.4 cycle achieved its goal. The H4 chain (H4.0 through H4.4) is
now closed end-to-end (read_file contract → continuation token binding
→ reconstruction → server chain delivery). H4.5 (TMPDIR remediation,
commit `57b44197`) is independent infrastructure hygiene tracked
elsewhere.

The umbrella `roadmap-auto-discovery` (superseded 2026-09-21) listed
h44 as Tier A "Concrete candidate — b-direct (H4.4 already merged;
archive the change)" — that classification was correct and this
archive-manifest executes it.
