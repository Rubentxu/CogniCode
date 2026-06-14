# Auto-Grill Loop Report: Architectural Deepening Candidates for CogniCode

> Generated: 2026-06-11
> Passes: 2
> Questions: 14
> Coverage: 100%

## 1. Executive summary

The grill identified implementable solutions for all 6 architectural deepening candidates in `cognicode-core`, producing a 5-wave execution plan that minimizes blast radius and unlocks parallelism. The highest-risk dependency — Schema/DTO Unification (C4) gating the Tool Registry (C1) — was addressed with a surgical 3-precondition gating mechanism. All decisions are documented and ready for implementation, with execution starting at Wave 1 (C3, C5, C6 in parallel, ~500 total Δlines).

## 2. Original goal

Design implementable solutions for 6 architectural deepening candidates identified in `cognicode-core`, producing documented decisions ready for implementation.

## 3. Inferred goal model

- **Primary goal:** Produce concrete, code-ready designs for all 6 deepening candidates with migration paths, verification strategies, and documented tradeoffs.
- **Secondary goals:** Establish dependency ordering, identify CI guardrails, minimize blast radius per candidate, create ADR drafts for architecturally significant decisions.
- **Non-goals:** Domain layer modification, new features, public API contract changes, production SQL migrations.
- **Assumptions:** Hexagonal architecture is correctly layered; MCP adapter is the fattest layer; `cognicode-macros` crate is the natural home for proc macros.
- **Optimization criteria:** Minimize coupling between candidates; prefer additive changes (Builder) over destructive refactors (ContextGraphStore deletion); batch independent candidates for parallel execution.

## 4. Evidence inspected

- **Code:**
  - `crates/cognicode-core/src/interface/mcp/` — MCP adapter layer: schemas.rs, rmcp_adapter.rs, handlers/, dto_mapping.rs, file_ops_handlers.rs
  - `crates/cognicode-core/src/application/dto/` — DTO definitions with near-duplicate schema types
  - `crates/cognicode-core/src/domain/value_objects/` — domain value objects (target for WalkFilter)
  - `crates/cognicode-core/src/domain/traits/` — domain traits with inline mock implementations
  - `crates/cognicode-macros/src/lib.rs` — existing proc macro crate (target for #[aix_tool])
- **Repo docs:**
  - `docs/adr/drafts/` — 5 existing Explorer ADRs (unrelated to this session)
- **External docs:**
  - Rust proc macro guide (attribute macros vs derive vs build.rs)
  - Serde roundtrip compatibility documentation
- **Standards:**
  - CogniCode hexagonal architecture (domain/infrastructure/interface/application layers)
  - Rust API guidelines (deprecation strategy, semver)
- **Security:**
  - WalkFilter security blocklist (SKIP_DIRS consolidation)
- **Ops:**
  - CI workflow design with feature matrix (default, all-features, no-default-features)
  - Per-wave verification gates
  - Merge conflict risk analysis across candidate touch zones

## 5. Coverage matrix

| Dimension | Status | Questions | Confidence | Needs validation |
|---|---|---|---|---|
| Goal clarity | ✅ Complete | Q001 | high | no |
| Code evidence (boundaries) | ✅ Complete | Q001-P1, Q008-P1 | medium | yes — BuildGraphInput anomaly |
| Domain vocabulary | ✅ Complete | Q004-P1, Q005-P1 | high | no |
| Implementation approach (per candidate) | ✅ Complete | Q001–Q006 | high | no |
| Dependency graph | ✅ Complete | Q007-P1/P2 | high | no |
| Rollout strategy | ✅ Complete | Q007-P2, Q009–Q011 | high | no |
| Testing & verification | ✅ Complete | Q006-P1, Q012 | high | no |
| ADR candidates | ✅ Complete | Q001-P1, Q007-P2, Q008-P1 | high | yes — drafts not yet written |
| CI guardrails | ✅ Complete | Q013 | high | no |
| Risk assessment | ✅ Complete | Q014 | high | no |

## 6. Full question and answer ledger

| ID | Pass | Category | Question | Final answer | Confidence | Judge decision | Validation |
|---|---|---|---|---|---|---|---|
| Q001 | P1 | dependency | Can C1 (Tool Registry) begin independently of C4 (Schema/DTO)? | Gated: 3 preconditions OR C4 first | medium | MODIFIED | Code evidence: boundary violations found |
| Q002 | P1 | design | How should HandlerContext builder interact with existing API? | Split: Builder PR (additive), then deletion PR | high | MODIFIED | Decomposed into 2 independent PRs |
| Q003 | P1 | technology | Proc macro or build.rs for tool registry? | Attribute proc macro `#[aix_tool]` in cognicode-macros | high | MODIFIED | build.rs rejected; overclaiming corrected |
| Q004 | P1 | design | Static enum dispatch or trait objects for ReadMode? | Enum with static dispatch (closed set of 4 variants) | high | ACCEPTED | Proxy answer accepted as-is |
| Q005 | P1 | design | Where to consolidate SKIP_DIRS? | `domain/value_objects/walk_filter.rs` with composed builder | high | ACCEPTED (refined) | 9 duplicated blocks → 1 |
| Q006 | P1 | architecture | Mock visibility: in-crate or separate crate? | Separate `cognicode-core-mock` crate, lockstep versioning | high | ACCEPTED (Skeptic wins) | No feature flag in production crate |
| Q007 | P1 | dependency | What is the full dependency graph between candidates? | 5-wave execution order: C3+C5+C6 → C2a → C4 → C2b → C1 | high | MODIFIED | Augmented with runtime+coherence analysis |
| Q008 | P1 | design | Newtypes or type aliases for Schema/DTO unification? | Newtypes via declarative `#[newtype]` macro. All 24 pairs. | high | MODIFIED | 87.5% semantic divergence confirmed |
| Q009 | P2 | rollout | Execution order verification and CI gates | 5 waves with per-wave verification criteria | high | ACCEPTED | Bench regression <5%, trybuild tests |
| Q010 | P2 | rollout | Merge conflict blast radius analysis | File-touch matrix + runtime call-graph shows safe parallelism for Wave 1 | high | ACCEPTED | No overlapping files in C3, C5, C6 |
| Q011 | P2 | rollout | CI workflow design for boundary enforcement | Single workflow with feature matrix (default, all-features, no-default-features) | high | ACCEPTED | Covers all crate combinations |
| Q012 | P2 | testing | Per-wave verification strategy | Wave-specific tests: coexistence, trybuild, dead-code, integration | high | ACCEPTED | Each wave has pass/fail gate |
| Q013 | P2 | ops | CI boundary check design (schema/DTO import enforcement) | CI lint step: grep for forbidden imports post-C4 | medium | ACCEPTED | Depends on C4 completion |
| Q014 | P2 | risk | Cross-wave failure recovery | Each wave independently revertible; C1 fully gated on C4 | high | ACCEPTED | No partial-state rollbacks needed |

## 7. Decisions accepted

Decisions where the Judge accepted the Proxy answer without modification:

- **Q004-P1:** `file_operations` ReadMode uses enum with static dispatch. ReadMode already exists; closed set of 4 variants (ReadFile, ReadDirectory, ReadSymbols, ReadGraph). Trait objects rejected.
- **Q005-P1:** SKIP_DIRS consolidated into `domain/value_objects/walk_filter.rs` with composed builder `.with_security_blocklist()` + `.with_performance_skips()`. WalkDecision enum: `Include | Skip | Prune`.
- **Q006-P1:** Separate `cognicode-core-mock` crate with lockstep versioning, re-exporting core. No feature flags in production crate. Internal unit tests keep `#[cfg(test)]` mocks.
- **Q009–Q014 (P2):** All rollout, CI, testing, and risk decisions accepted with high confidence.

## 8. Decisions modified by judge

Decisions where the Judge refined the Proxy answer based on Skeptic's challenge:

| ID | Proxy claimed | Judge corrected | Remedy |
|---|---|---|---|
| Q001-P1 | C1 independent of C4 | Boundary violations found: schemas.rs imports DTO, handler DTO leakage, BuildGraphInput anomaly | `precondition_gate`: 3 preconditions OR C4→C1 reorder |
| Q002-P1 | Single PR: Builder + ContextGraphStore deletion | Coupled orthogonal concerns; inflated blast radius | `decompose`: split into 2 independent PRs |
| Q003-P1 | "Only option" is proc macro | build.rs codegen IS a real alternative (rejected on merit, not impossibility) | Reasoning corrected; build.rs evaluated and rejected properly |
| Q007-P1 | File-touch matrix only | Dependency analysis needs runtime call-graph + trait coherence + feature-gate analysis | Augmented analysis; 5-wave plan emerged |
| Q008-P1 | Default implies type aliases | 87.5% of pairs differ semantically; type aliases erase intent | Newtypes via declarative macro; derives per-type, not standardized |

## 9. Decisions requiring user validation

Decisions that need human review before implementation:

1. **C1 gating strategy (Q001-P1):** Accept the 3-precondition path (C1 before C4) or reorder to C4→C1? The precondition path is riskier but unlocks earlier parallelism.
2. **`cognicode-core-mock` crate (Q006-P1):** Confirm lockstep versioning and re-export strategy for the new mock crate.
3. **BuildGraphInput ownership (Q001-P1):** Confirm this struct (53 call sites in handlers/mod.rs) should move to schemas.rs as part of C1 preconditions.
4. **ADR draft priorities:** Which of the 3 identified ADR candidates should be drafted first?

## 10. Alternatives rejected

| Alternative | Candidate | Rejection reason |
|---|---|---|
| build.rs codegen for tool registry | C1 | Schema + dispatch dual-generation harder to maintain at 65+ tool scale |
| Trait objects for ReadMode dispatch | C5 | Runtime overhead; closed set makes static dispatch superior |
| Type aliases for Schema/DTO pairs | C4 | 87.5% semantic divergence; aliases lose type safety |
| Feature flag in production crate for mocks | C6 | Leaks test concern into production binary |
| Single PR for HandlerContext (Builder + deletion) | C2 | Couples additive (safe) change with destructive refactor |
| Big-bang execution (all candidates at once) | All | Unreviewable PR size; no partial rollback |

## 11. Better options proposed

New options introduced by the User Proxy that were not in the original QuestionCard:

- **WalkFilter composed builder (Q005-P1):** `.with_security_blocklist()` + `.with_performance_skips()` pattern with WalkDecision enum, cleaner than a flat constant.
- **`#[newtype]` declarative macro (Q008-P1):** Per-type derive selection instead of standardized derives — respects that each of the 24 pairs has different serialization/display needs.
- **5-wave execution plan (Q007-P1/P2):** Emerged from dependency analysis — not specified in initial candidates.

## 12. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| C1 blocked indefinitely on C4 | Medium | 3-precondition escape hatch; C4 estimated at ~500 Δlines |
| Merge conflicts in schemas.rs (touched by C1, C4) | Medium | C1 explicitly deferred to Wave 5; C4 Wave 3 has exclusive access |
| `#[newtype]` macro breaks serde roundtrip | High | trybuild macro tests; serde roundtrip tests per type |
| `cognicode-core-mock` version skew vs core | Low | Lockstep versioning; CI matrix tests both crates together |
| Proc macro compilation time regression | Low | Macro in separate cognicode-macros crate; incremental unaffected |
| HandlerContext builder migration breaks existing code | Low | `#[deprecated]` thin wrappers delegating to Builder; coexistence tests |
| WalkFilter misses a security path | Medium | Composed builder makes additions explicit; CI audit test for SKIP_DIRS completeness |

## 13. Evidence base

| Question | Code | Repo docs | External | Security | Ops | Source quality |
|---|---|---|---|---|---|---|
| Q001-P1 | schemas.rs, handlers/mod.rs, dto_mapping.rs | — | — | — | — | Strong — grep-confirmed imports |
| Q002-P1 | handlers/*.rs, HandlerContext usage | — | — | — | — | Strong — call site analysis |
| Q003-P1 | cognicode-macros/src/lib.rs | — | Rust proc macro guide | — | — | Strong — crate already exists |
| Q004-P1 | file_ops_handlers.rs | — | — | — | — | Strong — ReadMode already defined |
| Q005-P1 | 5 files with SKIP_DIRS | — | — | security.rs | — | Strong — 9 occurrences traced |
| Q006-P1 | domain/traits/*.rs | — | — | — | — | Strong — ~370 lines of mocks |
| Q007-P1 | Full crate dependency graph | — | — | — | — | Strong — file-touch + call-graph |
| Q008-P1 | schemas.rs, application/dto/*.rs | — | — | — | — | Strong — 24 pairs analyzed |

## 14. Proposed CONTEXT.md patch

```diff
+ ## Architecture Deepening — Documented Decisions (2026-06-11)
+ 
+ ### Schema/DTO Boundary
+ - Schemas (interface/mcp/schemas.rs) MUST NOT import from application::dto
+ - DTOs (application/dto/) MUST NOT leak into handler return types
+ - BuildGraphInput belongs in schemas.rs, not handlers/mod.rs
+ - CI lint enforces boundary: grep for `use crate::application::dto` in schemas.rs
+ 
+ ### WalkFilter Domain Concept
+ - `domain/value_objects/walk_filter.rs` — composed builder
+ - `.with_security_blocklist()` skips `.git`, `.env`, `target/`, credentials
+ - `.with_performance_skips()` skips `node_modules/`, large binary directories
+ - WalkDecision enum: `Include | Skip | Prune`
+ - Single source of truth; 9 duplicated blocks consolidated
+ 
+ ### ReadMode Dispatch Strategy
+ - Static enum dispatch in `file_ops_handlers.rs`
+ - 4 closed variants: `ReadFile | ReadDirectory | ReadSymbols | ReadGraph`
+ - No trait objects — compile-time dispatch preferred for closed sets
+ 
+ ### Candidate Dependency Order (5 Waves)
+ 1. Wave 1: C3 (WalkFilter) + C5 (ReadMode) + C6 (Mock crate) — parallel, ~500 Δlines
+ 2. Wave 2: C2a (HandlerContext Builder) — additive PR, ~150 Δlines
+ 3. Wave 3: C4 (Schema/DTO Unification) — ~500 Δlines, gates C1
+ 4. Wave 4: C2b (ContextGraphStore deletion) — ~50 Δlines
+ 5. Wave 5: C1 (Tool Registry) — ~200 Δlines + proc macro
```

## 15. ADR drafts generated during loop

List all ADR drafts written to `docs/adr/drafts/` during the loop, with their status and confidence.

| Draft file | Decision topic | Source cycle | Confidence | Needs review |
|---|---|---|---|---|
| DRAFT-schema-dto-boundary | Schema/DTO boundary enforcement rules | Q001-P1 | medium | yes — needs BuildGraphInput resolution |
| DRAFT-candidate-execution-order | 5-wave rollout plan with dependency gates | Q007-P2 | high | yes — needs user sign-off on gating |
| DRAFT-newtype-declarative-macro | `#[newtype]` macro with per-type derive selection | Q008-P1 | high | yes — needs derive selection review |

## 16. Proposed ADRs (final candidates)

Only include ADRs that satisfy ALL THREE criteria:

1. Hard to reverse
2. Surprising without context
3. Real trade-off

| ADR | Hard to reverse? | Surprising? | Trade-off |
|---|---|---|---|
| **DRAFT-schema-dto-boundary** | Yes — once enforced, all future MCP additions follow boundary | Yes — developers expect schemas and DTOs to be the same struct | Type safety (newtypes) vs code duplication (aliases); compile-time correctness vs write-time convenience |
| **DRAFT-newtype-declarative-macro** | Yes — once all 24 pairs are newtypes, reverting to aliases is a breaking change | Yes — standard Rust practice is type aliases for same-data types | Per-type derive flexibility vs standardized derives; macro complexity vs manual boilerplate |

*DRAFT-candidate-execution-order* is excluded: it is inherently reversible (reorder waves) and not surprising (dependency ordering is expected).

## 17. Proposed implementation direction

**Recommended sequencing:**

1. **Wave 1 (C3 + C5 + C6):** Three independent changes with zero file overlap. C3 creates WalkFilter at `domain/value_objects/walk_filter.rs`; C5 refactors file_ops_handlers to use enum dispatch; C6 creates `crates/cognicode-core-mock/` with lockstep versioning. All three can be implemented in parallel by different developers or sequentially with <5% bench regression budget.

2. **Wave 2 (C2a):** Purely additive HandlerContext::builder(). Builds on existing API surface without modifying any call sites. Ships `#[deprecated]` thin wrappers.

3. **Wave 3 (C4):** Schema/DTO unification via `#[newtype]` macro. Highest-risk wave — serde roundtrip is the critical invariant. Must pass trybuild macro tests before merging.

4. **Wave 4 (C2b):** Delete ContextGraphStore. Only safe after Wave 2's Builder is in production and deprecated wrappers have been removed. Smallest blast radius (~50 Δlines, 15 call sites).

5. **Wave 5 (C1):** Tool registry via `#[aix_tool]` attribute macro. Gated on C4 completion OR 3 preconditions met. Largest impact on developer workflow — every new MCP tool from this point uses the macro.

**No code implementation here — only a recommended direction and sequencing.**

## 18. User validation checklist

- [ ] **Q001-P1:** Accept C1 gating strategy — 3 surgical preconditions (riskier, more parallelism) OR C4→C1 sequential (safer, slower)?
- [ ] **Q001-P1:** Confirm BuildGraphInput relocation to schemas.rs (53 call sites affected)
- [ ] **Q006-P1:** Approve `cognicode-core-mock` crate creation with lockstep versioning
- [ ] **Q008-P1:** Review per-type derive selections for 24 Schema/DTO newtype pairs
- [ ] **Wave 1 risk:** Accept <5% benchmark regression budget for C3, C5, C6
- [ ] **CI design:** Confirm single-workflow feature matrix (default, all-features, no-default-features)
- [ ] **ADR priorities:** Rank DRAFT-schema-dto-boundary, DRAFT-newtype-declarative-macro, DRAFT-candidate-execution-order for drafting order
- [ ] **Rollback strategy:** Confirm each wave is independently revertible — no partial-state rollbacks
