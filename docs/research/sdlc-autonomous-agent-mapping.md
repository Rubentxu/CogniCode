# SDLC Phases → Autonomous Agent Workflow Mapping

> Research document mapping the 7 classic SDLC phases to AI agent capabilities, inspired by
> Karpathy's AutoResearch, SDD (Spec-Driven Development), and the CogniCode ecosystem.

---

## 1. PLANIFICACIÓN (Planning)

### What the agent actually does

1. **Codebase archaeology**: Analyzes git log, file tree, dependency graph, and existing
   architecture to understand project scope and technical debt hotspots.
2. **Goal proposal**: Generates ranked list of improvement candidates (highest-impact bugs,
   most-changed modules, coverage gaps, complexity outliers).
3. **Effort estimation**: Scores each goal by estimated LoC touched, fan-out risk, and
   connascence distance.
4. **Roadmap generation**: Produces an ordered backlog with dependencies, using
   CogniCode's call graph to detect which changes must precede others.

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `git log --stat` analysis | Deterministic | Identify churn hotspots, bug-fix frequency per file |
| Issue tracker (GitHub/LINEAR) | Deterministic | Pull open issues, labels, priority |
| `cognicode_build_graph` | Deterministic | Call graph for dependency analysis |
| `cognicode_get_hot_paths` | Deterministic | Find most-called functions (highest fan-in) |
| `cognicode_get_entry_points` | Deterministic | Identify system boundaries |
| `cognicode-quality_analyze_project` | Deterministic | Code smell inventory, complexity baseline |
| `cognicode-quality_get_technical_debt` | Deterministic | SQALE debt quantification |
| `ripgrep` over codebase | Deterministic | Pattern-based architecture extraction |
| LLM (analysis) | LLM-assisted | Synthesize findings into narrative proposal |
| `engram_mem_search` | Memory | Check prior decisions and abandoned approaches |

### Metrics

- **Change frequency** (files modified > N times in last 90 days)
- **Bug density** (bugs per KLOC per module)
- **Code coverage gap** (% uncovered, per module)
- **Complexity hotspots** (cyclomatic > 15, cognitive > 20)
- **Technical debt ratio** (remediation cost / development cost)
- **Fan-out risk** (number of callers affected by change to a symbol)

### Gates

- [ ] Proposal references at least 3 data sources (not just LLM intuition)
- [ ] Each goal has a measurable success criterion
- [ ] Dependency graph shows no circular proposals
- [ ] Human sign-off on priority ordering

### What Karpathy's AutoResearch teaches us

Karpathy's `program.md` is the planning artifact. The human sets the **research direction**
("improve val_bpb on nanochat training"), and the agent owns the **what-to-try-next**
decision. Key lessons:

- **One metric to rule them all**: The binary keep/discard gate (`val_bpb improved? yes/no`)
  eliminates planning paralysis. For planning, this translates to: rank proposals by a
  single composite score (e.g., impact / risk).
- **Ratchet only forward**: The agent never revisits abandoned approaches unless the human
  resets the program. This forces empirical discipline.
- **Human sets direction, agent explores tactics**: The human writes the spec ("reduce
  overfitting"), the agent generates the hypothesis list ("add dropout", "reduce model
  width", "augment data").

---

## 2. ANÁLISIS DE REQUISITOS (Requirements)

### What the agent actually does

1. **Spec extraction from legacy code**: Reads existing code and generates a structured
   specification (inputs, outputs, invariants, edge cases) — the "code-to-contract" pattern.
2. **Gap detection**: Compares existing documentation/specs against implementation and
   flags discrepancies (undocumented endpoints, dead parameters, broken invariants).
3. **Ambiguity detection**: Flags underspecified requirements ("what happens when the list
   is empty?", "max length not defined").
4. **Scenario generation**: Produces Gherkin/BDD scenarios from code paths.
5. **Delta spec creation**: For new features, creates SDD delta specs using the existing
   spec as baseline.

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `cognicode_get_file_symbols` | Deterministic | Extract function signatures and contracts |
| `cognicode_get_outline` | Deterministic | Hierarchical view of module structure |
| `cognicode_get_symbol_code` | Deterministic | Extract source of specific functions for analysis |
| `cognicode_analyze_impact` | Deterministic | Understand blast radius of requirement changes |
| `cognicode_get_entry_points` | Deterministic | Map public API surface |
| LLM + `filesystem_read_text_file` | LLM-assisted | Compare docs/ vs impl/ for drift |
| `git diff` between docs and code changes | Deterministic | Detect documentation lag |
| `engram_mem_search` | Memory | Check if requirement was previously discussed/attempted |
| SDD skills (`sdd-spec`, `sdd-explore`) | Agent workflow | Structured spec creation |

### Metrics

- **Spec coverage**: % of public functions with documented contract
- **Doc-implementation drift**: Functions where signature differs from docs
- **Ambiguity count**: Underspecified edge cases detected
- **Requirement testability**: % of requirements with at least one test scenario
- **Invariant violations**: Code paths that break documented invariants

### Gates

- [ ] Every public API surface has a spec entry
- [ ] Zero documented-but-missing parameters
- [ ] Every spec scenario has at least one corresponding test (or flagged as untested)
- [ ] Breaking changes from baseline are explicitly listed

### How SDD feeds into this

The SDD workflow (`sdd-explore` → `sdd-propose` → `sdd-spec`) provides the structured
framework. The agent:

1. **Extracts baseline spec** from existing code (reverse-engineering)
2. **Generates delta spec** for the proposed change (only what changes)
3. **Runs gap analysis**: `sdd-spec` scenarios vs. `sdd-tasks` coverage
4. **Detects drift** in `sdd-verify`: implementation must match spec, not the other way
   around

The SDD principle: *"With AI-generated code, a code issue is an outcome of a gap in the
specification"*. The agent's job is to close those gaps autonomously.

---

## 3. DISEÑO (Design)

### What the agent actually does

1. **Architecture evaluation**: Generates call graph, detects cycles (Tarjan SCC),
   identifies god modules.
2. **Coupling analysis**: Computes connascence metrics (static: name, type, convention;
   dynamic: execution, timing, value, identity) for each module pair.
3. **Cohesion scoring**: Measures LCOM (Lack of Cohesion of Methods) per module.
4. **Dependency inversion check**: Flags violations of dependency inversion principle
   (low-level modules depending on even-lower-level, rather than abstractions).
5. **HLD generation**: Produces component diagrams and interface contracts.
6. **LLD generation**: Detailed class/method-level design with pre/post conditions.
7. **Alternative design exploration**: Generates 2+ architectural approaches with
   tradeoff analysis.

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `cognicode_check_architecture` | Deterministic | Cycle detection via Tarjan SCC |
| `cognicode_build_call_subgraph` | Deterministic | Module-level dependency graph |
| `cognicode_get_hot_paths` | Deterministic | Centrality analysis (god objects) |
| `cognicode_analyze_impact` | Deterministic | Coupling strength quantification |
| `cognicode-quality_analyze_complexity` | Deterministic | Per-function complexity metrics |
| `cognicode-quality_get_file_metrics` | Deterministic | File-level cohesion proxies |
| `cognicode_export_mermaid` | LLM-assisted | Visual architecture diagrams |
| LLM (design reasoning) | LLM-assisted | Connascence type classification, refactor suggestions |
| `cognicode-quality_get_technical_debt` | Deterministic | Baseline before design changes |

### Metrics (HLD)

| Metric | What it measures | Target |
|--------|-----------------|--------|
| SCC count | Number of cycles in call graph | 0 (or justified) |
| Fan-in variance | How evenly dependencies distribute | Low variance (no god modules) |
| Module depth | Layers in dependency tree | ≤ 5 layers |
| Abstractness / Instability | DIP adherence (Martin metrics) | Balanced |
| Distance from main sequence | A + I - 1 | Near 0 |

### Metrics (LLD — Connascence)

| Connascence type | Detected via | Refactoring direction |
|-----------------|-------------|----------------------|
| CoN (Name) — same variable names | Symbol cross-reference | Rename independently |
| CoT (Type) — shared types across modules | Type usage analysis | Move type to shared abstraction |
| CoC (Convention) — same magic values | Constant usage grep | Extract named constants |
| CoP (Position) — positional arg order | Function signatures | Replace with named/struct params |
| CoA (Algorithm) — duplicate logic | `cognicode-quality_detect_duplications` | Extract shared function |
| CoV (Value) — must-change-together values | Coupled change history in git | Extract configuration |
| CoE (Execution) — call order dependency | Call graph sequence analysis | Introduce state machine or guard |

**Connascence design rule**: *The stronger the connascence between two components, the
closer they should be (same module > same package > same layer > different service).*

### Gates

- [ ] Zero unhandled cycles in architecture (or explicitly documented exceptions)
- [ ] No new connascence type stronger than CoT introduced between modules > 2 layers apart
- [ ] Complexity delta ≤ 0 (no function increased in cyclomatic complexity without
  justification)
- [ ] Design document references at least 2 alternatives with tradeoffs

### What Karpathy teaches about design

The ratchet loop cannot do multi-step architectural refactors because each step must
individually improve the metric. This means:
- **Incremental design** works (extract interface, then refactor consumers)
- **Big-bang rewrites** fail (agent can't hold intermediate degraded state)
- The agent must decompose architectural changes into steps that each pass the gate

---

## 4. DESARROLLO (Development/Coding)

### What the agent actually does

This is the **core of the autoresearch loop**:

```
┌─────────────────────────────────────────────────────┐
│              AUTORESEARCH CODING LOOP                │
├─────────────────────────────────────────────────────┤
│  1. READ   — Understand context (symbols, callers)   │
│  2. PLAN   — Generate hypothesis (what to change)    │
│  3. MODIFY — Apply code change (bounded scope)       │
│  4. BUILD  — Compile check (hard gate)               │
│  5. TEST   — Run test suite (hard gate)              │
│  6. MEASURE— Evaluate metric (val_bpb equivalent)    │
│  7. KEEP   — git commit if ALL gates pass            │
│  8. DISCARD— git checkout if ANY gate fails          │
│  9. REPEAT — Go to 1 with new context                │
└─────────────────────────────────────────────────────┘
```

### Safety gates before applying changes

| Gate | Mechanism | Hard/Soft |
|------|-----------|-----------|
| Scope boundary | Agent can only modify files in the current change's scope | Hard |
| Compile check | `cargo build` / `go build` / `npm run build` | Hard |
| Lint gate | `cargo clippy` / `eslint` / `ruff` | Hard |
| Test gate | `cargo test` / `pytest` / `go test` | Hard |
| Quality diff | `cognicode-quality_get_quality_diff` — no new Critical/Blocker issues | Hard |
| Complexity diff | `cognicode-quality_analyze_complexity` — no function > 15 cyclomatic | Soft |
| Coverage floor | Line coverage must not decrease | Soft |
| Connascence ceiling | No new CoE (execution) or CoV (value) connascence across module boundary | Soft |
| Security scan | `cargo audit` / SAST tool | Hard for security fixes |
| Human review | Required when: safety-critical module, > 200 LOC, new public API | Hard for those cases |

### Scoping strategy

| Change scope | Agent autonomy | Human oversight |
|-------------|---------------|-----------------|
| Single function body | Full auto (Modify → Test → Keep/Discard) | None |
| Single file, multiple functions | Full auto with quality gate | Notification |
| Cross-file, same module | Auto with connascence analysis | Review required |
| New public API | Semi-auto (agent proposes, human approves interface) | Approval gate |
| Architecture refactor | Human design → agent implement step by step | Per-step review |
| Multi-module restructure | Human-led, agent assists | Full human oversight |

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `cognicode_get_symbol_code` | Deterministic | Read current function body |
| `cognicode_get_call_hierarchy` | Deterministic | Understand callers/callees before modifying |
| `cognicode_analyze_impact` | Deterministic | Preview blast radius |
| `cognicode_find_usages` | Deterministic | Find all references to rename/refactor |
| `filesystem_edit_file` | Deterministic | Apply code modification |
| `cargo build` / `go build` | Deterministic | Compile gate |
| `cargo test` / `go test` | Deterministic | Test gate |
| `cargo clippy` | Deterministic | Lint gate |
| `git commit` / `git checkout` | Deterministic | Keep/Discard mechanism |
| `cognicode-quality_get_quality_diff` | Deterministic | Regression detection |
| `cognicode-quality_analyze_file` | Deterministic | Smell check on modified file |

---

## 5. PRUEBAS (Testing)

### What the agent actually does

1. **Coverage gap detection**: Identifies uncovered code paths and generates tests.
2. **Regression test generation**: For each bug fix, generates a test that fails before
   the fix and passes after.
3. **Differential testing**: Runs old vs. new code on same inputs, compares outputs.
4. **Property-based test generation**: Generates PBT invariants from function signatures.
5. **Test quality assessment**: Evaluates mutation score, assertion density.
6. **Flaky test detection**: Re-runs tests N times, flags non-deterministic failures.
7. **Test suite as gate**: Every change must pass the full test suite.

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `cargo llvm-cov` / `coverage.py` | Deterministic | Coverage instrumentation |
| `cargo test` / `pytest` | Deterministic | Test execution |
| `cargo mutants` / `mutmut` | Deterministic | Mutation testing |
| `cognicode_get_file_symbols` | Deterministic | Find untested public functions |
| `cognicode_get_call_hierarchy` | Deterministic | Trace highest-risk code paths |
| `cognicode_trace_path` | Deterministic | Find execution paths for integration tests |
| LLM + `cognicode_get_symbol_code` | LLM-assisted | Generate test code for uncovered functions |
| `git diff` (old vs new behavior) | Deterministic | Differential testing input generation |
| `chronos_probe_start` + `chronos_query_events` | Deterministic | Runtime behavior comparison |
| `chronos_compare_sessions` | Deterministic | Old vs. new execution trace diff |

### Metrics

| Metric | Description | Gate threshold |
|--------|-------------|----------------|
| Line coverage | % lines executed by tests | ≥ 80% (module), must not decrease |
| Branch coverage | % branches covered | ≥ 70% |
| Mutation score | % mutants killed | ≥ 60% for new code |
| Test count per function | Tests per public function | ≥ 1 |
| Assertion density | Assertions per test | ≥ 1 |
| Flaky test rate | Tests that pass/fail non-deterministically | 0% |
| Test execution time | Wall clock for full suite | Must not increase > 20% |

### Gates

- [ ] Coverage does not decrease from baseline
- [ ] No Critical/Blocker quality issues in test code itself
- [ ] All new public functions have ≥ 1 test
- [ ] Regression test for each bug fix (test fails on old code, passes on new)
- [ ] Full test suite passes (hard gate)
- [ ] No new flaky tests introduced

### Differential testing: the Chronos connection

Using Chronos for runtime behavior comparison:
```
Baseline: chronos_probe_start old_binary → chronos_save_session → session_A
Candidate: chronos_probe_start new_binary → chronos_save_session → session_B
Comparison: chronos_compare_sessions(session_A, session_B) → diff report
```

This detects:
- Functions that changed call count (performance regression)
- Functions that changed call order (behavioral change)
- New/removed syscalls (capability change)
- Memory allocation differences

---

## 6. DESPLIEGUE (Deployment)

### What the agent actually does

1. **Deployment readiness validation**: Runs all quality gates, security scans, and
   integration tests.
2. **Changelog generation**: Produces human-readable changelog from conventional commits.
3. **Rollback plan generation**: Identifies which commits to revert and in what order.
4. **Canary decision**: Analyzes canary metrics (error rate, latency p99) and decides
   promote/rollback.
5. **Feature flag validation**: Checks that feature-flagged code paths are both tested
   (flag on AND flag off).
6. **Dependency audit**: Runs `cargo audit` / `npm audit` — blocks on Critical
   vulnerabilities.

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| `cognicode-quality_run_quality_gate` | Deterministic | Pre-deploy quality gate |
| `cognicode-quality_check_lint` (with all linters) | Deterministic | Multi-linter sweep |
| `cargo audit` / `npm audit` | Deterministic | Vulnerability scan |
| `cognicode-quality_get_technical_debt` | Deterministic | Pre-deploy debt snapshot |
| `cognicode_analyze_impact` | Deterministic | Blast radius of deploy |
| `git log --oneline` + LLM | LLM-assisted | Changelog generation |
| `chronos_performance_regression_audit` | Deterministic | Compare prod vs. staging performance |
| Canary metrics API | Deterministic | Error rate, latency, throughput |
| Feature flag service API | Deterministic | Toggle validation |

### Metrics

| Metric | Description | Gate threshold |
|--------|-------------|----------------|
| Critical vulnerabilities | CVEs with score ≥ 9.0 | 0 |
| High vulnerabilities | CVEs with score 7.0–8.9 | 0 |
| Quality gate status | All gates must pass | PASS |
| Technical debt delta | New debt introduced | ≤ 0 (no increase) |
| Performance regression | p99 latency increase | < 10% |
| Error rate canary | Error rate in canary vs. baseline | < 2x baseline |
| Feature flag coverage | Flagged paths tested in both states | 100% |

### Gates

- [ ] Quality gate: PASS (all conditions met)
- [ ] Security gate: Zero Critical/High CVEs
- [ ] Performance gate: No p99 regression > 10%
- [ ] Coverage gate: No decrease from baseline
- [ ] Rollback plan documented and validated
- [ ] Changelog reviewed by human (soft gate)
- [ ] Canary metrics green after N minutes (automated rollback if not)

---

## 7. MANTENIMIENTO (Maintenance)

### What the agent actually does

This is where **AutoResearch really shines** — the continuous improvement loop:

```
┌──────────────────────────────────────────────────────────────┐
│              CONTINUOUS MAINTENANCE LOOP                      │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐               │
│  │ MONITOR  │───▶│ DETECT   │───▶│ PRIORITIZE│              │
│  │ (logs,   │    │ (anomaly,│    │ (severity,│              │
│  │ metrics, │    │  crash,  │    │  impact,  │              │
│  │ errors)  │    │  smell)  │    │  effort)  │              │
│  └──────────┘    └──────────┘    └────┬─────┘               │
│       ▲                               │                      │
│       │          ┌──────────┐         │                      │
│       │          │ MEASURE  │◀────────┘                      │
│       │          │ (did it  │                                │
│       │          │  help?)  │                                │
│       │          └────┬─────┘                                │
│       │               │                                      │
│       │    ┌──────────┴──────────┐                          │
│       │    │                     │                           │
│       │    ▼                     ▼                           │
│       │  ┌──────┐           ┌──────────┐                    │
│       │  │ KEEP │           │ DISCARD  │                    │
│       │  │(git  │           │(git      │                    │
│       │  │commit)│          │revert)   │                    │
│       │  └──┬───┘           └──────────┘                    │
│       │     │                                                │
│       └─────┘                                                │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

### 7a. Bug detection from production

1. **Log anomaly detection**: Agent analyzes structured logs for error spikes, new error
   patterns, and correlation with deployments.
2. **Crash triage**: For each crash (SIGSEGV, panic), agent:
   - Identifies the crashing function from stack trace
   - Finds the commit that introduced the crash (`git bisect`)
   - Proposes a fix and a regression test
3. **Performance regression detection**: Compares Chronos traces from before/after deploy.

### 7b. Technical debt tracking and reduction

1. **Debt quantification**: Weekly run of `cognicode-quality_get_technical_debt` — tracks
   SQALE debt ratio trend.
2. **Debt prioritization**: Ranks issues by (remediation cost × impact).
3. **Auto-fix safe issues**: For low-risk smells (unused imports, simple formatting,
   naming conventions), agent auto-fixes and PRs.
4. **Refactor proposals**: For medium-risk issues (extract method, reduce complexity),
   agent creates proposal with diff preview for human review.

### 7c. Self-evolving rules (the CogniCode case)

The autonomous improvement loop already documented in the project:

```
Hypothesis → Modify rule → Evaluate (precision/recall/F1) → Keep/Discard → Repeat
```

Applied to code quality rules themselves:
- Agent modifies detection rule
- Runs against pinned evaluation corpus
- Compares against ground truth annotations
- Keeps if ALL of: ΔF1 > 0, ΔFPR ≤ 0, Δexec_time < +20%

### Tools needed

| Tool | Type | Purpose |
|------|------|---------|
| Log aggregation API (ELK/Loki) | Deterministic | Structured log querying |
| APM metrics (Datadog/Prometheus) | Deterministic | Error rate, latency, throughput |
| `cognicode-quality_get_quality_diff` | Deterministic | Detect new issues vs. baseline |
| `cognicode-quality_get_technical_debt` | Deterministic | Debt trend tracking |
| `cognicode-quality_get_remediation_suggestions` | Deterministic | Auto-fix candidates |
| `cognicode-quality_analyze_project` (weekly) | Deterministic | Full project health scan |
| `cognicode_analyze_impact` | Deterministic | Impact before applying auto-fix |
| `git bisect` automation | Deterministic | Root-cause commit identification |
| `chronos_debug_find_crash` | Deterministic | Crash analysis from traces |
| `chronos_performance_regression_audit` | Deterministic | Performance comparison |
| LLM (analysis) | LLM-assisted | Log pattern recognition, fix proposals |
| `engram_mem_save` | Memory | Record each fix for future reference |

### Metrics

| Metric | Description | Target |
|--------|-------------|--------|
| MTTR (Mean Time To Resolution) | Time from bug detection to fix deployed | Decreasing trend |
| Technical debt ratio | Remediation cost / development cost | Decreasing or stable |
| Bug reopening rate | Fixed bugs that recur within 30 days | < 5% |
| Auto-fix acceptance rate | % of auto-fixes merged without human changes | > 80% |
| Code smell count | Total smells detected weekly | Non-increasing |
| Performance regression count | Regressions introduced per deploy | 0 |
| Test suite health | % tests passing, flaky count | 100% pass, 0 flaky |

### Gates

- [ ] Auto-fixes must not introduce new Critical/Blocker quality issues
- [ ] Performance regressions auto-rollback (canary gate)
- [ ] Security patches skip human review gate (expedited path)
- [ ] Debt-increasing changes require explicit justification
- [ ] Every auto-fix has a corresponding test

---

## Cross-Cutting Patterns

### Pattern 1: The Ratchet Loop (from Karpathy)

```
MODIFY → EVALUATE → if BETTER: KEEP (commit) | if WORSE: DISCARD (revert) → REPEAT
```

Applicable to ALL phases, but with different metrics:
- **Planning**: Keeps proposals that survive analysis → discards infeasible ones
- **Design**: Keeps designs with lower connascence → discards higher-coupling alternatives
- **Coding**: Keeps changes that pass all gates → discards those that fail
- **Testing**: Keeps tests that increase mutation score → discards trivial tests
- **Deployment**: Keeps canary deployments with green metrics → auto-rollbacks red ones
- **Maintenance**: Keeps fixes that don't regress → discards ineffective ones

### Pattern 2: The Quality Gate Chain

Each phase has gates, and the agent cannot proceed past a gate until it's green:

```
PLANNING GATE ──▶ REQUIREMENTS GATE ──▶ DESIGN GATE ──▶
CODING GATE ──▶ TESTING GATE ──▶ DEPLOYMENT GATE ──▶ MAINTENANCE GATE (loop)
```

If a later phase fails, the agent backtracks to the earliest phase that could fix it:
- Test failure → back to Coding
- Design flaw discovered in Coding → back to Design
- Requirement gap found in Testing → back to Requirements

### Pattern 3: Progressive Autonomy

| Phase | Autonomy level |
|-------|---------------|
| Planning | AI-assisted (human sets direction) |
| Requirements | AI-led (human reviews) |
| Design | AI-led for LLD, AI-assisted for HLD |
| Coding | Full auto for bounded changes |
| Testing | Full auto (with human review of test logic) |
| Deployment | AI-led with automated rollback |
| Maintenance | Full auto for safe fixes, AI-assisted for refactors |

### Pattern 4: The Memory Backbone

Every phase writes to Engram:
- **Planning**: "We decided to prioritize module X because of Y"
- **Requirements**: "Spec for endpoint Z extracted, gap found in error handling"
- **Design**: "Connascence analysis shows strong CoV between A and B"
- **Coding**: "Fixed bug in function F, root cause was X"
- **Testing**: "Added coverage for edge case E, mutation score improved by 5%"
- **Deployment**: "Deployed v1.2.3, canary metrics green after 10min"
- **Maintenance**: "Auto-fix applied to smell S134 in file F"

Future sessions use `engram_mem_search` to avoid repeating mistakes and to build on
prior decisions.

---

## Concrete Example: End-to-End Flow

### Scenario: "Reduce false positives in rule S134"

#### Phase 1 — Planning
- Agent analyzes `cognicode-quality_list_smells` → S134 has highest FPR (0.12)
- Agent checks git log: S134 touched 4 times in last month, all manual fixes
- Agent proposes: "Autonomous exploration to lower S134 FPR below 0.05"
- **Gate**: Human approves → move to Requirements

#### Phase 2 — Requirements
- Agent reads S134 source via `cognicode_get_file_symbols` and `cognicode_get_symbol_code`
- Agent extracts current behavior: "Flags functions where nesting depth > X"
- Agent identifies gap: "Threshold X is hardcoded, not calibrated against corpus"
- Agent creates delta spec: "S134 must detect nesting violations with precision > 0.90
  AND recall > 0.85 on rust corpus"
- **Gate**: Spec is testable (has concrete metric) → move to Design

#### Phase 3 — Design
- Agent generates 2 approaches:
  A) "Calibrate threshold based on corpus statistics"
  B) "Replace regex with AST depth calculation"
- Connascence analysis: Approach A has CoV (threshold value shared across callers),
  Approach B has CoA (algorithm must match tree-sitter grammar)
- Agent recommends A (lower connascence strength, faster)
- **Gate**: Design doc with tradeoffs → move to Coding

#### Phase 4 — Coding (AutoResearch Loop)
```
Iteration 1: Threshold 4→5 → ΔF1: +0.02, ΔFPR: -0.03 → KEEP (commit)
Iteration 2: Threshold 5→6 → ΔF1: +0.01, ΔFPR: -0.01 → KEEP (commit)
Iteration 3: Threshold 6→7 → ΔF1: -0.05, ΔFPR: -0.08 → DISCARD (revert)
Iteration 4: Add exclusion for test files → Compile error → DISCARD (revert)
Iteration 5: Keep exclusion + fix syntax → ΔF1: +0.03, ΔFPR: -0.02 → KEEP (commit)
```
- **Gate**: Auto-keep 3 improvements, discard 2 failures → auto-PR with results.tsv

#### Phase 5 — Testing
- Agent generates regression tests for each kept change
- Agent runs mutation testing: 87% mutation score (gate: ≥ 60%) → PASS
- Agent runs full test suite: 142/142 pass → PASS
- Agent validates on held-out corpus files: precision 0.92, recall 0.86 → meets spec
- **Gate**: All metrics above thresholds → move to Deployment

#### Phase 6 — Deployment
- Quality gate run: PASS (no new Critical/Blocker issues)
- Security scan: PASS (no new CVEs)
- Performance: S134 execution time +3% (gate: < 20%) → PASS
- Canary: Deploy to 10% of analysis workers, error rate stable for 15min → PROMOTE
- **Gate**: All gates green → full deploy

#### Phase 7 — Maintenance
- Weekly debt scan: S134 FPR now 0.04 (was 0.12) → recorded as improvement
- Agent writes to Engram: "S134 optimized via threshold calibration + test exclusion"
- Next week: Agent detects S3776 FPR regression → loops back to Phase 1 for S3776
- Continuous: Every week, agent picks top-N worst-performing rules and runs the loop

---

## Summary: Phase-to-Agent Mapping

| SDLC Phase | Agent Role | Core Metric | Hard Gate | AutoResearch Loop? |
|------------|-----------|-------------|-----------|-------------------|
| Planning | Analyzer + Proposer | Impact/Risk score | Human sign-off | No (direction-setting) |
| Requirements | Extractor + Gap detector | Spec coverage % | Testable scenarios | No (baseline creation) |
| Design | Evaluator + Generator | Connascence distance | Cycle-free architecture | Partial (explore alternatives) |
| Coding | Auto-modifier | val_bpb equivalent | Build + Test + Lint | YES (core loop) |
| Testing | Generator + Validator | Mutation score | Coverage non-decreasing | YES (test improvement) |
| Deployment | Gate runner | Error rate | Security + Quality + Perf | YES (canary evaluation) |
| Maintenance | Continuous improver | Trend direction | No regressions | YES (continuous loop) |

## References

- Karpathy, A. (2026). [autoresearch](https://github.com/karpathy/autoresearch) — Autonomous AI research loop
- GitHub Spec Kit. [Spec-Driven Development](https://github.com/github/spec-kit)
- PwC (2026). [Agentic SDLC in Practice](https://www.pwc.com/m1/en/publications/2026/docs/future-of-solutions-dev-and-delivery-in-the-rise-of-gen-ai.pdf)
- Microsoft. [AI-led SDLC with Azure and GitHub](https://techcommunity.microsoft.com/blog/appsonazureblog/an-ai-led-sdlc-building-an-end-to-end-agentic-software-development-lifecycle-wit/4491896)
- Page-Jones, M. (1992). Comparing Techniques by Means of Encapsulation and Connascence. *Communications of the ACM*.
- connascence.io — Connascence as a software design metric
- ai-sdlc.io — Open-source AI agent orchestrator for full SDLC
