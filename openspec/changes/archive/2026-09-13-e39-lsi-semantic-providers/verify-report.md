```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:e8e558014ce14580a12d1c65ebf549f2ce7226ff52b18bef08d6d8e11ab46950
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 6/6
scenarios: 8/8
test_command: just lsi-providers
test_exit_code: 0
test_output_hash: sha256:087d9f03fd28b3cd2ca6106451648a060dbe50d56fec687f3a9c8afa4c8344c6
build_command: cargo check -p cognicode-core --features evidence-kernel
build_exit_code: 0
build_output_hash: sha256:e3221344350c8043adffddac5875048bb509a5533a4f969682d9a4140126561e
```

## Verification Report

**Change**: e39-lsi-semantic-providers
**Version**: N/A (delta specs carry no version field)
**Mode**: Standard

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 22 |
| Tasks complete | 22 |
| Tasks incomplete | 0 |

Tasks.md is 22/22 `[x]` (phases 1–5). Note: `state.yaml` still shows `phases.apply.status: pending` — stale orchestrator bookkeeping, not a task artifact (see SUGGESTION-4).

### Build & Tests Execution
**Build**: ✅ Passed

```text
cargo check -p cognicode-core                            EXIT 0
cargo check -p cognicode-core --features evidence-kernel EXIT 0
cargo check -p cognicode-explorer                        EXIT 0
cargo check -p cognicode-runtime                         EXIT 0
cargo check -p cognicode-core-mock                       EXIT 0
just lint       (clippy -p cognicode-runtime --bin explorer-api -- -D warnings) EXIT 0
cargo fmt --check                                        EXIT 0 (empty output)
cargo clippy -p cognicode-core --lib [--features evidence-kernel] -- -D warnings EXIT 0
cargo clippy -p cognicode-core --all-targets --features evidence-kernel          EXIT 0 (zero warnings)
cargo clippy -p cognicode-explorer -- -D warnings                                 EXIT 0
```

**Tests**: ✅ 125 passed / ❌ 0 failed / ⚠️ 1 environment-gated branch not exercised (Java live-server)

```text
just lsi-providers                                             EXIT 0
  provider_conformance   8 passed  (rust 5, ts 5, java unavailable branch, 3 synthetic)
  cp5_tie_break          3 passed
  fact_bridge (gated)   22 passed
cargo test -p cognicode-core --lib code_intelligence                        14 passed (both feature states)
cargo test -p cognicode-core --lib composite                                10 passed (both feature states; ~43s)
cargo test -p cognicode-core --lib batch_builder --features evidence-kernel  9 passed
cargo test -p cognicode-core --lib lsp_facts   --features evidence-kernel    7 passed
cargo test -p cognicode-core --lib continuity  --features evidence-kernel   36 passed (dependency closure)
cargo test -p cognicode-core --test cp5_tie_break --features evidence-kernel 3 passed
just lsi-fixtures check      EXIT 0   RESULT: PASS — all 42 goldens byte-identical
just lsi-equivalence         EXIT 0   7 passed; python-hello/rust-hello scores 1.0000/1.0000/1.0000
just lsi-identity            EXIT 0   identity_benchmark 7 passed + workspace_isolation 2 passed
```

**Per-language conformance observations (fresh, from `just lsi-providers`)**

```text
rust (serverless, lsp policy-off): get_symbols S0, get_document_symbols S0, hover S0,
  find_references S0, get_definition S1 -> all Served, declarations matched
ts   (serverless, lsp policy-off): same shape; S0 x4 + S1, declarations matched
java (jdtls absent -> declared-unavailable): get_symbols S0, hover S0 (each carrying
  "lsp@S2 unavailable: Failed to spawn jdtls"), get_definition Unresolved
  (lsp@S2 unavailable + local-resolver@S1 degraded); no S2-tier result; suite passed
```

**Regression digests**: `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` and `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` are unchanged (`equivalence_harness/harness.rs`, `identity_benchmark/harness.rs` are outside the e39 diff); the matcher digest printed by the fresh identity run matches the pin, and the equivalence entity-identity pin check passed.

**Evidence hashes** (preimage of `evidence_revision` = sorted `sha256␠filename` lines of these captures):

| Command | Log sha256 |
|---|---|
| `just lsi-providers` | `087d9f03fd28b3cd2ca6106451648a060dbe50d56fec687f3a9c8afa4c8344c6` |
| `--lib code_intelligence` | `6dbb8ca56f15a0aa6cf5b9bac3c980fc17fa63041e9cbc5a18078e2af0f57b9c` |
| `--lib composite` | `af4b1e33b8fbc8f566603c9fb170810618d434af4b25c4ea0a1db9f84b41e98b` |
| `--lib batch_builder --features evidence-kernel` | `1965a7a6a2a9e2a612cee96fc98690c0ce6ba040d49ef0d4ce5f65982e565f29` |
| `--lib lsp_facts --features evidence-kernel` | `b60cb118be0fc22074a8d93ee1e150c5b9623ee5725f31c5d8e397efd3dd1aaa` |
| `--lib fact_bridge --features evidence-kernel` | `45dd7032278f9042467c9f7f82be7c62037fe360fb7cbca0000d15dfe9b8485e` |
| `--lib continuity --features evidence-kernel` | `d5f822483defd7f371171e50e9cbf00ef20162534e868feb8a837834b48576a9` |
| `--test cp5_tie_break --features evidence-kernel` | `047548ddb5c798e12a7aacb6792ee8f72a6e61f554932c61b671f23caa5b7dbb` |
| `just lsi-fixtures check` | `24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc` |
| `just lsi-equivalence` | `69e4d1d117a3e603f3335682b842f869a8b3b1b4da69d03234430ed3ee6e125e` |
| `just lsi-identity` | `849fa6e1da02e5653b7607ed4d290ad9a9fb25e7c1cf5c486c851c6dd0dcc852` |
| `cargo check` core ± features / explorer / runtime / core-mock | `e3221344…`, `7c06affc…`, `3eb9cc8b…`, `a6839658…` |
| `just lint` / `cargo fmt --check` | `95fc9e7d…` / `e3b0c442…` (empty) |
| clippy scoped (lib gated/ungated, explorer, all-targets) | `20a2c9b9…`, `e3221344…`, `9eb35956…`, `4055f1a8…` |

**Coverage**: ➖ Not available — no coverage command or threshold configured for this change (`openspec/config.yaml` declares no `verify.coverage_threshold`).

### Spec Compliance Matrix

Authoritative e39-owned surface: `specs/provider-tier-provenance` (4 reqs / 5 scenarios) + `specs/provider-pipeline-conformance` (2 reqs / 3 scenarios) = **6 requirements / 8 scenarios**.

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Ordered policy-gated tier pipeline | Gated or failed tier falls through | `composite.rs > test_gated_lsp_tier_falls_through_declaring_serving_tier` (S2 policy-off → S0 served, no diagnostic, S2 counters 0/0) | ✅ COMPLIANT |
| Structured diagnostics and counters travel with results | Fallback diagnostic is attached and counted | `composite.rs > test_fallback_diagnostic_is_attached_and_counted` (S2 `Unavailable` diagnostic naming provider+tier+outcome, S0 served, S2 counters 1/0/1) | ✅ COMPLIANT |
| Pinned tier-to-provenance mapping | Tier decides provenance class | `batch_builder.rs > tier_decides_provenance_class` + `lsp_facts.rs > tier_decides_provenance_class` (S1 → Inferred, detail `tier=S1 provider=local-resolver`) | ✅ COMPLIANT |
| Heuristic results never claim Extracted and unresolved never fabricates | Ambiguous heuristic match stays Ambiguous | `batch_builder.rs > s0_heuristic_observations_are_ambiguous_never_extracted` (class Ambiguous, `assert_ne!(Extracted)`) + lsp_facts `maps_reference_kinds_and_hierarchy_to_canonical_predicates` S0 assertions | ✅ COMPLIANT |
| Heuristic results never claim Extracted and unresolved never fabricates | Unresolved site propagates without a fact | `lsp_facts.rs > unresolved_symbol_query_records_site_and_exhausted_tiers_without_a_fact` (+ `unresolved_symbol_queries_name_the_symbol_fact_side_fqn`): zero facts, site + exhausted tiers recorded | ✅ COMPLIANT |
| Declared per-tier precision targets | Contradicting tier fails the run | `provider_conformance.rs > contradicting_tier_fails_the_run` (failure names query, `declared tier S2`, `observed tier S0`) | ✅ COMPLIANT |
| Availability-gated language coverage | Java without its server degrades by declaration | `provider_conformance.rs > java_without_its_server_degrades_by_declaration` (fresh run: jdtls absent → S2 `Unavailable` diagnostics, no S2-tier result, suite passes) | ✅ COMPLIANT |
| Availability-gated language coverage | Matching declarations pass without servers | `provider_conformance.rs > rust_matching_declarations_pass_without_servers` + `ts_matching_declarations_pass_without_servers` (S0×4+S1 each, no thresholds) | ✅ COMPLIANT |

**Compliance summary**: 8/8 scenarios compliant (6/6 requirements).

**Supplementary rows (not counted in the envelope totals)**

| Spec surface | Coverage | Result |
|---|---|---|
| Umbrella `semantic-provider-pipeline` — "Fallback is visible" | composite counters/diagnostics test + conformance runner declarations | ✅ |
| Umbrella `semantic-provider-pipeline` — "Unresolved call" | `lsp_facts` unresolved-record tests (no target fact) | ✅ |
| Umbrella requirement clause "provider id/version" | provider **id** + tier recorded (`tier=<T> provider=<id>`); no provider **version** notion exists anywhere in the pipeline | ⚠️ PARTIAL (pre-existing umbrella gap; e39 closed the id/tier half) |
| `lsi-m0-baseline` (3/6) | `just lsi-fixtures check` — 42/42 byte-identical | ✅ |
| `projection-equivalence-harness` (4/8) | `just lsi-equivalence` — 7/7; identity pin unchanged | ✅ |
| `identity-benchmark` (2/3) | `just lsi-identity` — gates precision/recall/line-shift/move all 1.0000 | ✅ |
| `entity-continuity` (4/6) | `--lib continuity --features evidence-kernel` 36/36 + CP-5 tie-break 3/3 | ✅ |
| `generic-graph-projection` (2/4) | dual-gated suite, untouched by e39 diff; exercised through equivalence harness | ✅ (evidence reused) |

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| Ordered policy-gated tier pipeline | ✅ Implemented | `TIER_ORDER [S2,S1,S0]`, `TierPolicy`, `with_policy`; per-op attempts run S2 → (S1) → S0; gated/unsupported tiers skipped without diagnostic or counter; `get_definition` keeps LSP `Ok(None)` authoritative (historical rule). |
| Structured diagnostics and counters travel with results | ✅ Implemented | `ProviderDiagnostic{provider, attempted_tier, outcome, message}` attached to `Tiered::diagnostics`; per-tier `AtomicU64` counters exposed via `status() -> CompositeStatus`; `warn!`/`debug!` per attempt. |
| Pinned tier-to-provenance mapping | ✅ Implemented | `PrecisionTier::provenance_class()` (S2→Extracted, S1→Inferred, S0→Ambiguous) is the only derivation used by `finish()`; `add_tiered_observation` rejects a contradicting declared class with `FactBridgeError::TierProvenanceContradiction`; detail composes `tier=<T> provider=<id>` after existing entries. No new `ProducerKind`, no new predicate (kernel `evidence_kernel/` and `value_objects/` untouched by the diff). |
| Heuristic results never claim Extracted and unresolved never fabricates | ✅ Implemented | S0 facts always `Ambiguous`; exhausted queries (`get_symbols`, `find_references`, `get_hierarchy`; former silent `Err(_) => continue/return` sites at lsp_facts old :116/:167/:205) call `record_unresolved` with site/query/exhausted_tiers and emit no fact, no subject/target fabrication (container fallback remains the documented non-joinable file path). |
| Declared per-tier precision targets | ✅ Implemented | `sandbox/fixtures/lsi-providers/{rust,ts,java}/expected.json` loaded before running; `harness::verify` fails on tier or outcome contradiction naming query/declared/observed; matches pass with no numeric threshold. |
| Availability-gated language coverage | ✅ Implemented | Rust/TS run `with_policy(lsp=false)` serverless; Java PATH-probes `jdtls` (no spawn) and runs the declared-unavailable branch when absent (fixture 5s readiness bound), asserting no S2-tier result plus an `Unavailable` diagnostic. |

### Coherence (Design)
| Decision | Followed? | Notes |
|----------|-----------|-------|
| D1 tier types in `domain/traits/code_intelligence.rs`, additive tiered trait, no serde | ✅ Yes | `PrecisionTier{S0..S4}` with Display/FromStr, `ProviderOutcome`, `ProviderDiagnostic`, `Tiered`, `TieredOutcome`; no `Serialize`/`Deserialize` on any of them; original 6-op trait and all `dyn` consumers compile unchanged. |
| D2 composite pipeline `[S2,S1,S0]` + per-op support matrix + `TierPolicy` + old-trait mapping + `FallbackResult` removal | ✅ Yes (mechanism notes) | `FallbackResult` and its 2 tests removed; old-trait maps `Unresolved` per op. Order is encoded per-op (if-blocks) rather than by iterating `TIER_ORDER`; `TIER_ORDER` feeds `status()` and the doc comment overstates "walks TIER_ORDER" (SUGGESTION-2). |
| D3 counters atomics + `CompositeStatus`; consumers = runner + unit tests only (A1) | ✅ Yes | `TierCounters`/`CompositeStatus` implemented; consumed by composite unit tests (runner asserts declarations, not counters — see SUGGESTION-1); no MCP/CLI surface change, so response shapes/goldens stay stable. |
| D4 tiered `add_provider`, class derivation in `finish()`, contradiction check in `finish()` | ⚠️ Partial | Class derivation in `finish()` ✅; contradiction check moved to `add_tiered_observation` admission (WARNING-1; spec sentence still satisfied — see Adjudication). |
| D5 `UnresolvedRecord` + `take_unresolved()`, `finish()` signature unchanged, exhaustion replaces silent continues | ✅ Yes | Exactly as designed; `finish(self) -> Vec<Fact>` unchanged, unresolved records in-memory only, three exhaustion sites record. |
| D6 ungated runner + fixtures with declared targets, Java PATH probe, fixed-argv recipe | ✅ Yes | Runner ungated; fixtures under `sandbox/fixtures/lsi-providers/`; probe is a scan (`path_probe_scans_without_spawning`); `just lsi-providers` fixed argv. |
| D7 ungated pipeline, default-path stability | ✅ Yes | 42/42 goldens byte-identical, MCP/CLI signatures untouched, feature-gated mapping stays gated. |
| D8 CP-5 `cp5_tie_break.rs` gated file | ✅ Yes | 3 tests: canonical sort, SubjectIndex tie-break, continuity first-defines agree. |

### Deviation Adjudication (flagged items)

| Item | Finding | Rule |
|---|---|---|
| (a) D4-vs-D5: contradiction check at observation admission, `finish()` signature unchanged | Spec sentence: "A class contradicting its declared tier MUST fail batch construction." Admission returns `Err(TierProvenanceContradiction)` before any record is pushed; `finish()` cannot receive a contradictory record (it is the only tier-attributed insertion path) and its test proves the rejected observation leaves no trace. Failing earlier in construction satisfies and strengthens the spec; the spec does not name `finish()` or its return type. | **Spec holds.** Mechanism deviates from D4's wording → WARNING-1, not spec-breaking. |
| (b) Old-trait `Unresolved → Ok(None)` for definition/hover swallows `FileNotFound`/`InvalidLocation` | Real default-path delta: pre-e39 a missing file surfaced `Err(FileNotFound/InvalidLocation)` from the fallback (CLI printed `Error: …`, exit non-zero); now CLI prints "No definition/hover information found" with exit 0, and MCP already mapped `Err(_)` to `found:false` so its shape is unchanged. The other four ops still return `Err(Internal(exhausted summary))` (error preserved, variant collapsed). No spec (e39 delta, umbrella, or promoted main specs) pins this error taxonomy; D2 explicitly prescribes `Ok(None)` for both ops. | **Not spec-covered; implementation follows D2's explicit mapping.** WARNING-2 for the CLI/MCP UX change and the imprecise "today's shapes" rationale. |
| (c) `get_hierarchy` S2-first readiness wait (up to 30s default) | Spec-mandated by REQ1 fixed highest-first order; D2 names it as a behavior delta (pre-e39 hard-coded the fallback). The bound is the pre-existing `wait_timeout_secs` used by the other location ops; the composite suite exercises the gated-off path instantly and the S2-attempt path (~43s incl. readiness exits). Risk: first hierarchy query without a server now pays the readiness bound before falling through. | **Coherent with design and spec.** Observation/latency WARNING-3, not a deviation. |
| (d) `provider_id` derived from a static tier→identity map | `lsp_facts::tier_provider_id` (S2→`lsp`, S1→`local-resolver`, S0→`tree-sitter`) is pinned by `tier_provider_ids_match_the_provider_identity_constants`, so a rename on either side fails the test. With exactly one provider per tier in M4 this cannot mis-attribute; it would only drift if a tier were ever served by multiple providers. | **Acceptable; pinned.** SUGGESTION-3 (prefer the diagnostic-carried provider id if a tier ever multiplexes). |
| (e) `PrecisionTier` no-serde; S3/S4 reserved → `Extracted` unreachable | Verified: no serde derives on any new type; `TierPolicy::enabled` returns `false` for S3/S4 unconditionally and no producer constructs them, so the reserved→Extracted arm is doc-marked unreachable and unreachable in practice. Tier identity crosses persistence only as detail strings; kernel/predicate surfaces untouched. | **Compliant with D1 and the bincode rule.** |
| (f) Counters unconsumed outside tests/runner (A1) | `CompositeStatus`/`status()` are referenced only from composite unit tests; the conformance runner consumes declarations, not counters. A1 resolved counters as internal+tracing, and REQ2 only requires counters "queryable from composite status", which the passing unit assertions prove. D3's sentence listing the runner as a consumer overstates actual usage. | **Spec satisfied.** SUGGESTION-1 for the design-doc wording. |

### Issues Found
**CRITICAL**: None.

**WARNING**:
1. **W1 — D4 mechanism deviation**: the tier/class contradiction check lives in `add_tiered_observation` (admission) instead of `finish()` as D4 worded it. Spec sentence "fail batch construction" still holds (no contradictory batch can be constructed); recorded so the design doc and code agree after archive.
2. **W2 — legacy error UX change on the default path**: `get_definition`/`hover` now return `Ok(None)` when every tier is exhausted, swallowing `FileNotFound`/`InvalidLocation` failures that previously surfaced as `Err` (CLI: error + non-zero exit → informational line + exit 0). Not covered by any spec; D2 prescribes the mapping but its "today's shapes" rationale is inaccurate for error paths. The other four ops preserve an error (`Internal` summary, variant collapsed from `FileNotFound`/`InvalidLocation`/`ParseError`).
3. **W3 — `get_hierarchy` default-path latency**: an S2 readiness attempt (bounded by `wait_timeout_secs`, default 30s) now precedes tree-sitter for hierarchy, where the old code never touched LSP for this op. Design-named and spec-demanded, but it is a user-visible latency change on the default MCP/CLI path when no server is installed.

**SUGGESTION**:
1. **S1**: D3 lists the conformance runner as a counters consumer; the runner asserts declarations only. Either consume `status()` in the runner or correct the design wording.
2. **S2**: `TIER_ORDER` is used for `status()` ordering only; the doc comment "Each op walks `TIER_ORDER`" describes intent, not the per-op hardcoded sequence.
3. **S3**: Prefer carrying the serving provider id on the observation rather than deriving it from the tier→identity map, should any tier ever host more than one provider.
4. **S4**: `state.yaml` `phases.apply.status`/`phases.verify.status` still read `pending` while tasks are 22/22; orchestrator to refresh at archive time. Also `sandbox/fixtures/lsi-providers/` and the two new test files are still untracked — commit with the change.

### Honest Gaps
- **Java live-server branch not exercised locally**: `jdtls` is absent from PATH, so `java_lsp_targets_verify_when_the_server_is_available` reports an environment skip; the S2-declared Java manifest path (S2-served results, `Served` outcomes) has no fresh runtime evidence in this environment. The spec-mandated behaviour for this environment is the declared-unavailable branch, which passed. Consistent with resolved assumption A3.
- **Formal UAT deferred (A5)** — recorded in the proposal/state as a post-M4 dedicated cycle; no UAT evidence claimed.
- **SCIP/LSIF spike criteria recorded only** (M5+, out of scope); S3/S4 have no producer and no tests by design.
- **GAP S2 generic-projection equivalence harness** remains on the ledger; `multi-lang-types` is quarantined in the equivalence run as expected (reported, not scored).
- **Umbrella "provider id/version" clause** is only half-satisfied (id + tier recorded; no version notion exists in-repo) — pre-existing umbrella surface, not introduced by e39.

### Verdict
**PASS WITH WARNINGS**
All 22 tasks complete; 6/6 requirements and 8/8 e39 scenarios compliant with fresh runtime evidence; build, lint, fmt, clippy, fixtures (42/42), equivalence and identity gates all green. Three non-spec-breaking warnings (contradiction-check mechanism, legacy error UX swallowing on definition/hover, hierarchy readiness latency) plus one pre-existing umbrella gap and one un-exercised environment-gated Java branch.
