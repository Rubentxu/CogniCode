# Archive Report: e30-corpus-expansion

**Change**: e30-corpus-expansion
**Archived**: 2026-08-06
**Branch**: `feat/e30-corpus-expansion` (HEAD 20a6e31b, 13 commits)
**Mode**: engram (artifacts in Engram; openspec main specs updated directly)
**Verdict**: PASS_WITH_WARNINGS (Batch A 6/6 + Batch B: G2 68/68, runtime 29 scenarios 0 errors, tier3 rust-analyzer 2/2 pass, smoke 0 regressions)
**Debt verdict**: PASS_WITH_WARNINGS (R1, C3.1 closed, DQS 63)

---

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| release-readiness-gate | Updated | G2 denominator dynamically resolved to 68 (runtime tools/list); MCP-TOOLS.md regeneration requirement incorporated |
| sandbox-validation-system | Updated | Tools-List Pagination Probe + MCP-TOOLS Documentation Regeneration requirements already present in main spec (lines 156–172); corpus expansion requirements incorporated |

### Delta → Main Spec Merge Summary

**`release-readiness-gate/spec.md`** — syncs complete:
- G2 denominator corrected: 43 (stale hardcode) → 68 (runtime tools/list output via `list_mcp_tools.sh`)
- `generate_tool_coverage.py` integration confirmed: dynamic denominator not hardcoded
- MCP-TOOLS.md regeneration requirement incorporated

**`sandbox-validation-system/spec.md`** — already current:
- `Tools-List Pagination Probe` (lines 156–163): PAGE_SIZE=20, base64 offset cursor, complete collection
- `MCP-TOOLS Documentation Regeneration` (lines 165–172): runtime as source of truth, not hand-maintained
- Tier-1 corpus: tokio, clap added to ripgrep, serde, anyhow
- Tier-3 corpus: rust-analyzer, typescript, react added (100k+ LOC, scale lane)
- Ground-truth matcher extension: `symbols_min` / `has_result` count-only mode

---

## Archive Contents

> **Mode note**: This change used `engram` mode — all SDD artifacts (proposal, specs, design, tasks, verify-report, debt-report) live as Engram observations and were NOT materialized to `openspec/changes/`. The archive report here serves as the audit trail for the openspec-main-spec sync and knowledge graph updates.

| Artifact | Observation ID | Status |
|----------|----------------|--------|
| proposal.md | — | Engram (topic: `sddk/e30-corpus-expansion/proposal`) |
| spec.md (delta) | obs-d0c0d2cda25c41f2 (#6021) | Engram |
| design.md | — | Engram (topic: `sddk/e30-corpus-expansion/design`) |
| tasks.md | — | Engram (topic: `sddk/e30-corpus-expansion/tasks`) |
| verify-report | obs-93d1770d8b915ea9 (#6023) | Engram |
| debt-report | obs-4ecb91f64bd5d2b5 (#6024) | Engram |
| archive-report.md | — | `openspec/changes/archive/2026-08-06-e30-corpus-expansion/archive-report.md` (this file) |

---

## Deliverables Summary — All Gates Passed

| Deliverable | Evidence |
|-------------|----------|
| **G2 denominator dynamic = 68** | `sandbox/scripts/list_mcp_tools.sh` paginates PAGE_SIZE=20 → `total: 68`; `generate_tool_coverage.py` reads runtime not hardcode |
| **G2 68/68 with runtime evidence** | `coverage_matrix.yaml` 68 tools covered; `awk '{print $2}'` on matrix confirms 68/68 |
| **Paginated probe** | `list_mcp_tools.sh` collects all tools without truncation at first page; `nextCursor` absent at end |
| **MCP-TOOLS.md regenerated** | `docs/MCP-TOOLS.md` regenerated from runtime `tools/list` surface; runtime declared as source of truth |
| **Coverage generator** | `generate_tool_coverage.py` produces `coverage_matrix.yaml` with per-tool family and coverage status |
| **Scorecard** | `just release-scorecard` emits scorecard.json/scorecard.md covering G1–G12 |
| **27 SHAs pinned** | `sandbox/scripts/clone_repos.sh` pins 27 repos to exact 40-hex SHAs |
| **7 new manifests** | `tokio_repos.yaml`, `clap_repos.yaml`, `tier3_rust_repos.yaml`, `tier3_typescript_repos.yaml`, `tier3_react_repos.yaml`, `coverage_fill.yaml`, `zod_repos.yaml` (zod collision resolved) |
| **Corpus Tier-1 expansion** | tokio + clap added to existing ripgrep/serde/anyhow |
| **Corpus Tier-3 expansion** | rust-analyzer + typescript + react (100k+ LOC each) |
| **zod collision resolved** | `sandbox/repos/zod` — 0 typescript/zod refs; ts_repos.yaml updated |
| **Matchers count-only + tests** | `symbols_min` / `has_result` count-only mode; `ground_truth.rs` extended |
| **29 runtime scenarios, 0 errors** | Batch B: `coverage_fill.yaml` + tokio + clap; rust-analyzer 2/2 pass |
| **smoke 0 regressions** | `just sandbox-ci-smoke` exit 0 |

---

## Git

- **Branch**: `feat/e30-corpus-expansion`
- **HEAD**: `20a6e31bd50ffab6d9d24302b850dc58ee5fd645`
- **Commits**: 13
- **Artifacts path**: Engram (topic: `sddk/e30-corpus-expansion/*`)

---

## Debt Follow-ups Carried Forward

| Severity | Item | File | Effort | Pre-existing? |
|----------|------|------|--------|----------------|
| **WARN** | S1.5: Tier-3 container resource limits not elevated (still 2G/128 for rust, 1G/64 for js/ts; design wanted 4G/256) | `sandbox/containers/*.container` | M | No |
| **WARN** | C3.3: `js_repos.yaml` / `ts_repos.yaml` `pinned_sha` field holds TAG strings (v5.1.0, v11.0.0) while `clone_repos.sh` uses real SHAs | `sandbox/manifests/{js,ts}_repos.yaml` | S | **Yes** (pre-existing, B-direct hotfix main) |
| **WARN** | C3.8: `clone_repos.sh` sleep/delay between clones not tuned for rate-limit tolerance | `sandbox/scripts/clone_repos.sh` | XS | No |
| **WARN** | D2.2: `generate_tool_coverage.py` `families` dict has 32 dead tool names (brain_\*, explorer_\*, impact_\*) not in runtime 68 | `sandbox/scripts/generate_tool_coverage.py` | S | No |
| **WARN** | D2.3: `pin_all_shas.sh` is orphan (zero call sites in justfile or workflows) | `sandbox/scripts/pin_all_shas.sh` | XS | No |
| **SUGG** | S1.6: `has_any_results` is module-scope while `count_symbols` is nested inside `score_mermaid` (inconsistent) | `sandbox-core/src/ground_truth.rs` | S | No |
| **SUGG** | S1.7: `symbols_min` / `has_result` count-only matchers lack unit tests | `sandbox-core/src/ground_truth.rs` | M | No |
| **SUGG** | C3.4: `scorecard` recipe uses `|| true` + dead `coverage_exit=$?` capture | `sandbox/justfile: release-scorecard` | XS | No |

**C3.1 (schema↔manifest arg drift)** — **CLOSED** in this cycle: `detect_god_functions max_lines` vs schema `min_lines`; `generate_contract/validate_contract` arg-shape mismatches; `reparse_on_edit` singular vs Vec. All remediated in remediation round 0→1 on same branch.

---

## Knowledge Graph Updates

### Cycle Node Created
- Path: `~/.sddk-knowledge/cognicode/cycles/CYC-2026-08-06-e30-corpus-expansion.md`
- Linked to: milestone `[[M-E30-Fase-2]]`, spec `[[release-readiness-gate]]`, spec `[[sandbox-validation-system]]`, ADR `[[ADR-031]]`, ADR `[[ADR-032]]`

### Milestone Created
- `M-E30-Fase-2`: status → `completed`, closed date → `2026-08-06`

### ADRs Referenced / Superseded
- ADR-031 (Definition of 1.0.0): G2 denominator amended from 43 to 68 — **ADR text still stale** (L20/L35/L44 say "43"); carry-forward warning for ADR maintenance
- ADR-032 (sandbox-validation-system): Tier-1/Tier-3 corpus expansion validated — **ADR text still stale** (L18/L104 say "43 tools"); carry-forward warning for ADR maintenance

---

## Jurisprudence Candidate

**No** — verdict is PASS_WITH_WARNINGS (not clean PASS). The correction cycle (remediation round 0→1 for C3.1) and the pre-existing js/ts_repos.yaml pinned_sha tag-vs-SHA drift mean no clean-first-pass decision crystallized. The G2=68/68 resolution is a reusable architectural decision, but its primary record is the Engram decision observation (#6020, topic `sddk/e30-corpus-expansion/g2-denominator`).

---

## Release Handoff

```yaml
ready_for_release: true
change: e30-corpus-expansion
branch: feat/e30-corpus-expansion
merge_policy: guided  # debt follow-ups + ADR maintenance attached to PR body
next_recommended: sddk-release
```

**Risk note**: ADR-031 and ADR-032 still hardcode "43 tools" despite openspec reflecting 68. Recommend ADR maintenance patch before or alongside PR merge.

---

*Archive generated by sddk-archive (GLM-4.7) — 2026-08-06*
