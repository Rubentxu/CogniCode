```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:89aa41314a754fabd439c237fc6f10e76d658edaa3662d483901ffca5bef6672
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 6/6
scenarios: 8/8
test_command: just lsi-providers
test_exit_code: 0
test_output_hash: sha256:d32491121d83c7db15f66544f014b89bf5f39961e16b36530dd2ff9a377a4ffa
build_command: cargo check -p cognicode-core --features evidence-kernel
build_exit_code: 0
build_output_hash: sha256:55d360de722098a896f65d6efc282f19e59649f54dea0abd977b75e7d21441ac
```

## Verification Report

**Change**: e39.1-provider-ux-hardening
**Version**: N/A (no delta specs; behavior corrections under the e39 delta contract)
**Mode**: Standard

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 20 |
| Tasks complete | 20 |
| Tasks incomplete | 0 |

Tasks.md is 20/20 `[x]` across phases 1–5 (W2 RED/GREEN, W3 RED/GREEN, guards + handoff). The change root carries `proposal.md` + `tasks.md` only: no `specs/`, no `design.md`, no `state.yaml`. The absence of delta specs is by construction — the proposal declares `New: None. Modified: None` and flags e39's W2 as "Not spec-covered"; e39.1 sharpens implementation semantics under the already-verified e39 contract.

### Build & Tests Execution
**Build**: ✅ Passed
```text
cargo check -p cognicode-core                                              EXIT 0
cargo check -p cognicode-core --features evidence-kernel                   EXIT 0
cargo check -p cognicode-core --features evidence-kernel,multimodal        EXIT 0
cargo check -p cognicode-explorer                                          EXIT 0
cargo check -p cognicode-runtime                                           EXIT 0
cargo check -p cognicode-core-mock                                         EXIT 0
just lint     (clippy -p cognicode-runtime --bin explorer-api -- -D warnings) EXIT 0
cargo fmt --check                                                          EXIT 0 (empty output)
cargo clippy -p cognicode-core --lib --tests -- -D warnings                EXIT 0 (zero findings on touched paths)
cargo clippy -p cognicode-core --lib --tests --features evidence-kernel -- -D warnings EXIT 0
cargo clippy -p cognicode-core --test provider_conformance -- -D warnings  EXIT 0
```

**Tests**: ✅ 121 passed / ❌ 0 failed / ⚠️ 1 environment-gated branch skipped (Java live-server)
```text
cargo test -p cognicode-core --lib composite                              16 passed (W2: 4 new/reworked; W3: 4 new; e39 guards)
cargo test -p cognicode-core --lib code_intelligence                      14 passed
just lsi-providers                                                        EXIT 0
  provider_conformance   8 passed   (rust 5, ts 5, java unavailable branch, 3 synthetic)
  cp5_tie_break          3 passed
  fact_bridge (gated)   22 passed
just lsi-fixtures check                                                   EXIT 0  RESULT: PASS — all 42 goldens byte-identical
just lsi-equivalence                                                      EXIT 0  7 passed; scores byte-identical
just lsi-identity                                                         EXIT 0  7 + 2 passed; gates all 1.0000
cargo test -p cognicode-core --lib continuity --features evidence-kernel   36 passed (main-spec guard)
cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal  6 passed
```

**Per-language conformance observations (fresh, from `just lsi-providers`)**
```text
rust (serverless, lsp policy-off): get_symbols S0, get_document_symbols S0, hover S0,
  find_references S0, get_definition S1 -> all Served, declarations matched
ts   (serverless, lsp policy-off): same shape; S0 x4 + S1, declarations matched
java (jdtls absent -> declared-unavailable): get_symbols S0, hover S0 (each carrying
  "lsp@S2 unavailable: Failed to spawn jdtls"), get_definition Unresolved
  (lsp@S2 unavailable + local-resolver@S1 degraded); no S2-tier result; suite passed
java (available branch): PATH-probe skipped ("skipped: jdtls is not on PATH")
```

**Regression digests**: `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` printed by the fresh identity run equals the pin; `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` is unchanged and its `verify_identity_pin` test passed inside `just lsi-equivalence`. `git status` shows no changes under `sandbox/` — no golden, score, or digest re-pin.

**Evidence hashes** (preimage of `evidence_revision` = sorted `sha256␠filename` lines of these 19 captures):

| Command | Log sha256 |
|---|---|
| `cargo test -p cognicode-core --lib composite` | `6fcaf3cd56db3f42a94c00537d1c79533c0a9135ac6ee2e8f7ce1e06d65dab5f` |
| `cargo test -p cognicode-core --lib code_intelligence` | `fd7455f7a5bddf873bc4d26b8e9eaead7c736461ab59ec59a88aba1639c8caf1` |
| `just lsi-providers` | `d32491121d83c7db15f66544f014b89bf5f39961e16b36530dd2ff9a377a4ffa` |
| `just lsi-fixtures check` | `24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc` |
| `just lsi-equivalence` | `441b90b3c1317726bbeec82d4e5cb35231d234b07f88945b31edcecc05b33389` |
| `just lsi-identity` | `8932d4b87dfbe80b3a5989589aa0b4af94a15cc0852e43d4ce19982748c96558` |
| `--lib continuity --features evidence-kernel` | `829d3622d8b7b078b5febdb6479069ba1c4b2aeb7ce0415841f1afbc987c382b` |
| `--lib generic_graph_projection --features evidence-kernel,multimodal` | `ea61f36e3f2a9c7789d737cc7c6bc1601049f7744fd24d76a06570e6fbaf2427` |
| `cargo check` core default / `--features evidence-kernel` / `+multimodal` | `b8305cd8…` / `55d360de…` / `55d360de…` |
| `cargo check` explorer / runtime / core-mock | `55f4ab40…` / `80ee3b59…` / `b8305cd8…` |
| `just lint` / `cargo fmt --check` | `a6ad820c…` / `e3b0c442…` (empty) |
| clippy core default / core gated / provider_conformance | `e5546b58…` / `94abf687…` / `439f0e96…` |

**Coverage**: ➖ Not available — no coverage command or threshold configured (`openspec/config.yaml` declares no `verify.coverage_threshold`).

### Spec Compliance Matrix

e39.1 writes no deltas; the authoritative surface it modifies behavior under is the e39 delta contract: `openspec/changes/e39-lsi-semantic-providers/specs/provider-tier-provenance/spec.md` (4 requirements / 5 scenarios) + `.../provider-pipeline-conformance/spec.md` (2 / 3) = **6 requirements / 8 scenarios** (recounted from the files). All covering tests ran fresh after the e39.1 diff and passed at runtime.

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Ordered policy-gated tier pipeline | Gated or failed tier falls through | `composite.rs > test_gated_lsp_tier_falls_through_declaring_serving_tier` (S2 policy-off → S0 served + declared, no diagnostic, S2 counters 0/0) + fresh rust/ts conformance runs | ✅ COMPLIANT |
| Structured diagnostics and counters travel with results | Fallback diagnostic is attached and counted | `composite.rs > test_fallback_diagnostic_is_attached_and_counted` (S2 `Unavailable` naming provider+tier+outcome, S0 served, counters 1/0/1) + fresh Java unavailable run diagnostics | ✅ COMPLIANT |
| Pinned tier-to-provenance mapping | Tier decides provenance class | `batch_builder.rs > tier_decides_provenance_class` + `lsp_facts.rs > tier_decides_provenance_class` (S1 → Inferred, detail `tier=S1 provider=local-resolver`) — fact_bridge 22/0 | ✅ COMPLIANT |
| Heuristic results never claim Extracted and unresolved never fabricates | Ambiguous heuristic match stays Ambiguous | `batch_builder.rs > s0_heuristic_observations_are_ambiguous_never_extracted` (class Ambiguous, `assert_ne!(Extracted)`) | ✅ COMPLIANT |
| Heuristic results never claim Extracted and unresolved never fabricates | Unresolved site propagates without a fact | `lsp_facts.rs > unresolved_symbol_query_records_site_and_exhausted_tiers_without_a_fact` (+ `unresolved_symbol_queries_name_the_symbol_fact_side_fqn`): zero facts, site + exhausted tiers recorded | ✅ COMPLIANT |
| Declared per-tier precision targets | Contradicting tier fails the run | `provider_conformance.rs > contradicting_tier_fails_the_run` (failure names query, declared tier, observed tier) | ✅ COMPLIANT |
| Availability-gated language coverage | Java without its server degrades by declaration | `provider_conformance.rs > java_without_its_server_degrades_by_declaration` (fresh: jdtls absent → S2 `Unavailable` diagnostic per query, no S2-tier result, suite passes) | ✅ COMPLIANT |
| Availability-gated language coverage | Matching declarations pass without servers | `provider_conformance.rs > rust_matching_declarations_pass_without_servers` + `ts_matching_declarations_pass_without_servers` (S0×4+S1 each, no thresholds) | ✅ COMPLIANT |

**Compliance summary**: 8/8 scenarios compliant (6/6 requirements).

**Supplementary rows (not counted in the envelope totals) — five promoted main specs regression via their guards**

| Main spec | Guard (fresh) | Result |
|---|---|---|
| `lsi-m0-baseline` | `just lsi-fixtures check` — 42/42 byte-identical | ✅ |
| `projection-equivalence-harness` | `just lsi-equivalence` — 7/7, scores byte-identical, identity pin holds | ✅ |
| `identity-benchmark` | `just lsi-identity` — 7+2, gates precision/recall/line-shift/move all 1.0000, matcher digest unchanged | ✅ |
| `entity-continuity` | `cargo test -p cognicode-core --lib continuity --features evidence-kernel` — 36/36 | ✅ |
| `generic-graph-projection` | `cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal` — 6/6 | ✅ |

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| W2 — deepest-attempted-provider error propagation | ✅ Implemented | `Attempt::Failed(ProviderDiagnostic, Option<CodeIntelligenceError>)` carries the originating error at every S2/S1/S0 attempt site (`composite.rs:493-775`). `definition_chain`/`hover_chain` (`:781-897`) track `DeepestAnswer`; old-trait `get_definition`/`hover` (`:948-986`) map `Errored(e) → Err(e)`, `Miss → Ok(None)`. Verified the pre-e39 (`87993f90^`) passthrough: LSP-not-ready returned `fallback.<op>()` directly and a ready server's `Err` propagated — the original `FileNotFound`/`InvalidLocation` variants are restored. |
| W2 — clean miss / gate-only stays `Ok(None)` | ✅ Implemented | `test_clean_miss_and_gate_only_failure_stay_ok_none` covers S1 clean miss, terminal S0 hover miss, and unsupported-language gate-only failure; `test_local_resolver_none_is_unresolved_and_maps_to_ok_none` covers the S1 `Ok(None)` path. |
| W2 — collection ops unchanged | ✅ Implemented | `get_symbols`/`find_references`/`get_hierarchy`/`get_document_symbols` still map `Unresolved → Err(Internal("all tiers exhausted (…)"))` (`:911-974`); `test_symbols_unresolved_maps_to_internal_error` and `test_hierarchy_attempts_s2_before_s0_and_maps_to_internal_error` pass. No CLI/MCP edits (proposal scope honored). |
| W3 — outer readiness bound | ✅ Implemented | `gate_lsp_language` computes `policy.readiness_budget(wait_timeout_secs, fallback_tier)` and wraps the WHOLE readiness call: `readiness_within(budget, self.wait_for_lsp_ready(...))` (`:442-489`) — bounding `LspProcess::initialize`'s own 30s `REQUEST_TIMEOUT_SECS` (`process.rs:12`) that the poll loop could not. `readiness_budget = full.min(fallback_readiness)` when the fallback tier is enabled, full wait otherwise; `DEFAULT_FALLBACK_READINESS = 2s`; per-op fallback tiers S1 for definition, S0 for the other five (matches the support matrix). |
| W3 — diagnostic, counters, re-spawn | ✅ Implemented + documented | Timeout maps to an S2 `Unavailable` diagnostic `"…LSP readiness exceeded the {budget}s bounded fallback to {tier}"` and falls through; `test_hierarchy_falls_through_within_the_bounded_readiness` (rust-analyzer present on PATH → real spawn) asserts the bounded diagnostic and S2 counters `(1,1)`, S0 `(1,1)`; `test_bounded_readiness_cuts_a_hanging_readiness` proves the wrapper cuts a pending future. Re-spawn documented in-code (`:460-465`) and confirmed against `process_manager.rs`: registration in `processes` happens only after `initialize` returns, and `Drop for LspProcess` calls `child.start_kill()`, so a bounded timeout drops/kills the unregistered attempt and a later query re-spawns from `Starting`. |
| e39 S2 doc fix; `TierPolicy` not persisted | ✅ Implemented | `TIER_ORDER` doc now states the per-op matrix walk + `status()` row ordering. `TierPolicy` still derives only `Debug, Clone, Copy, PartialEq, Eq` — no serde; bincode rule intact. |
| Conformance Java pins | ✅ Implemented | Unavailable branch = `fallback_readiness 5s`; available branch = `fallback_readiness 30s` (equals the full wait, so the declared-S2 run gets full readiness). Rust/TS serverless policy switched to `..TierPolicy::all()`. |

### Coherence (Design)
No `design.md` exists for e39.1 (proposal + tasks only), so design coherence is skipped by construction; the change is a correction slice under the e39 design D2/D3 semantics.

Deviation adjudication (all four items requested by the assignment):

| Item | Finding | Rule |
|---|---|---|
| (a) Review-budget overrun | Authored diff = **839 changed lines** (`composite.rs` +635/−178 = 813; `provider_conformance.rs` +21/−5 = 26) vs the 400-line budget and the tasks forecast of 150–220 with `400-line budget risk: Low` / `Decision needed before apply: No`. Measured composition: production 559 lines (316 non-comment, non-blank additions), tests 280; whitespace-insensitive diff is still 587/130. The TESTING-STATE handoff note "the excess is mostly the 12 mandated RED/guard tests (~310 lines)" is inaccurate — tests are 280 of 839 and production alone exceeds the budget. Delivery was declared `auto-chain` single slice with no `size:exception`, so `sdd-apply` started oversized work without the guard's required resolution. Semantic change is small and correct, but reviewer load is real. | Process WARNING, not spec-breaking. |
| (b) Cold start can degrade to S1/S0 within 2s, retried later | Coherent with the e39 deltas. "Gated or failed tier falls through" explicitly covers unavailable/failed tiers falling through; a bound expiry is recorded as S2 `Unavailable` with a structured diagnostic and counters, the serving tier is declared, and no scenario pins a 30s S2 wait. The one place full readiness was implicitly required — the Java available-branch declared-S2 run — is explicitly pinned to 30s. Proposal risk table accepts the tradeoff with mitigation (later queries retry; bound configurable). | Accepted; coherent with e39 deltas. |
| (c) `with_wait_timeout` unused in-repo | Confirmed zero call sites (`grep` over `crates/`), no dead-code warning because it is `pub`. Doc note ("the bounded fallback of the policy still applies") keeps the semantics explicit. | SUGGESTION only. |
| (d) Java available-branch 30s pin | `min(30s, 30s) = 30s` = full readiness, so the declared-S2 run is preserved verbatim; no behavior loss. Assert-by-construction in this environment (jdtls absent). | Accepted. |

### Issues Found
**CRITICAL**: None.

**WARNING**:
1. **W-1 — review-budget guard overrun**: 839 authored changed lines vs the 400-line default budget (2.1×) and the 150–220 forecast; no chained/stacked PR slice and no accepted `size:exception`. Also, the handoff note's "mostly tests" characterization is not supported by measurement (production 559, tests 280). Requires a `size:exception` note or slice split at PR time; do not silently reforecast.

**SUGGESTION**:
1. **S-1**: `CompositeProvider::with_wait_timeout` now has zero in-repo callers; consider a deprecation/removal note at the next API window (public API intentionally kept per apply record).
2. **S-2**: The working tree also carries unrelated uncommitted doc edits (`docs/ROADMAP.md` +55/−32, `docs/V1.0.0-PRE-CUT-CHECKLIST.md` +100/−42) outside the proposal's declared affected areas; keep the e39.1 commit scoped to the two source files + `.agent/TESTING-STATE.md`.
3. **S-3**: The Java unavailable-branch 5s pin is not timing-exercised here (jdtls absence fails the spawn immediately), so the bound itself is runtime-proven only by the rust-analyzer-backed unit test with a 50ms bound; a PATH-independent hanging-server fixture would close that gap if the 5s pin must be runtime-evidenced.
4. **S-4**: The change root has no `state.yaml`; orchestrator bookkeeping is absent (not required by verification, but archive/state continuity will need it).

### Honest Gaps
- **Java live-server (available) branch not exercised**: `jdtls` is absent from PATH, so `java_lsp_targets_verify_when_the_server_is_available` reports an environment skip; the 30s full-readiness pin and the declared-S2 Java run have no fresh runtime evidence in this environment.
- **Java 5s unavailable bound not timing-exercised**: the spawn-failure path returns immediately; only the S2 readiness diagnostic path is observed.
- **Real ~2s cold-start degradation not observable locally**: the bounded fall-through is proven with a 50ms bound against a real rust-analyzer spawn, not with the 2s default.
- **Coverage**: no threshold/command configured → not available.
- **Unrelated doc churn** (see S-2) is unverified by this report and must not be counted as e39.1 scope.

### Verdict
**PASS WITH WARNINGS**
All 20 tasks complete; 6/6 requirements and 8/8 e39 scenarios compliant with fresh runtime evidence; build, lint, fmt, clippy, fixtures (42/42), equivalence, identity, continuity and dual-gated projection guards all green. One non-spec-breaking process warning (839-line authored diff vs the 400-line review budget, no `size:exception`) plus three suggestions and recorded environment-gated gaps.
