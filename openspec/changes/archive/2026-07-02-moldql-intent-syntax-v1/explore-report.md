# Kernel Exploration: moldql-intent-syntax-v1

## Context Quality

- **Level: C1** — Implementation is well-understood (parser, AST, executor, compile all read), but the *intent syntax* concept itself is aspirational and undocumented in code. A documented-vs-implemented contradiction exists (see below).
- **Evidence Present:**
  - `crates/cognicode-explorer/src/moldql/` — full parser, AST, executor, compile (4 files, ~3000 lines)
  - `crates/cognicode-explorer/assets/moldql-scaffolds.yaml` — 9 scaffolds using "intent" syntax
  - `crates/cognicode-explorer/src/ask/patterns.rs` — separate NL regex router (8 patterns)
  - `CONTEXT.md` lines 267-269 — canonical MoldQL examples
  - Memory #3830 — claims "MoldQL Syntax Levels" definition was added to CONTEXT.md
  - Memory #1447 — ExplorerQL grammar proposal (already implemented)
- **Missing Context:** No ADR for intent syntax. No spec. The "Syntax Levels" definition claimed by memory #3830 is **NOT present** in CONTEXT.md (grep returns nothing for "Syntax Levels", "intent-first", "graph-primitive"). This is a memory-vs-reality contradiction.
- **Recommended Effort: deepen** — the gap is concrete and the fix surface is identifiable, but scope boundaries need sharpening before proposal.

## Current State

MoldQL today has **two disjoint query surfaces** that do not connect:

### 1. Structured MoldQL (actually implemented, parser-tested)

The hand-written recursive-descent parser (`parser.rs`, 876 lines) accepts **7 uppercase leading keywords**:

| Keyword | Syntax | Status |
|---------|--------|--------|
| `FIND` | `FIND <target> [IN SCOPE <path>] [WHERE <conds>] [APPLY <lens>]` | ✅ Executing |
| `EXPLORE` | `EXPLORE <obj_ref> THROUGH <callers|callees> DEPTH <n>` | ✅ Executing |
| `PATH` | `PATH FROM <from> TO <to> [MAX HOPS <n>] [WHERE ...]` | ✅ Parses + compiles, petgraph stub returns empty |
| `NEIGHBORS` | `NEIGHBORS <root> DEPTH <n> [DIRECTION <d>] [WHERE ...]` | ✅ Parses + compiles, petgraph stub |
| `SUBGRAPH` | `SUBGRAPH ROOT <root> [DEPTH <n>] [DIRECTION <d>] [WHERE ...]` | ✅ Parses + compiles, petgraph stub |
| `CLUSTER` | `CLUSTER [METHOD (scc|connected)] [WHERE ...]` | ✅ Parses + compiles, petgraph stub |
| `EXPLAIN` | `EXPLAIN FROM <from> TO <to> [WHERE ...]` | ✅ Parses + compiles, petgraph stub |

Plus boolean composition: `(Q1 AND Q2)`, `Q1 OR Q2`, `NOT Q`.

**Execution reality:** FIND and EXPLORE fully execute against `SymbolRepository` + `GraphQueryPort`. The 5 ExplorerQL primitives compile to PG SQL (parameterized) and petgraph plans, but `run()` returns **empty results** for petgraph (`run_petgraph_plan` is a stub) and **FeatureDisabled** for PG. So ExplorerQL is grammar-complete but execution-incomplete.

### 2. "Intent" / lowercase syntax (documented everywhere, parsed nowhere)

CONTEXT.md (lines 267-269) shows these as *the* MoldQL examples:
```
symbols where kind = "function" and fan_out > 5
calls from "UserService::create_user" depth 3
docs citing adr "ADR-008"
```

This lowercase English style is used in **9 production locations** but **the parser rejects all of it** — it expects uppercase `FIND`/`EXPLORE`/etc. as the first token. Confirmed grep hits using unparseable intent syntax:

- `CONTEXT.md` — canonical examples (3)
- `assets/moldql-scaffolds.yaml` — all 9 `query_template` values
- `apps/explorer-ui/.../ViewSpecWizard.tsx` — placeholder + help examples
- `apps/explorer-ui/e2e/viewspec-authoring.spec.ts` — test input
- `apps/explorer-ui/.../ScaffoldPicker.test.tsx` — expected query strings
- `apps/explorer-ui/src/api/schemas.test.ts` — ViewSpec data_source fixtures
- `crates/cognicode-explorer/src/dto.rs` — test fixtures (3)
- `crates/cognicode-explorer/src/boundary.rs` — test fixture
- `crates/cognicode-explorer/tests/viewspec_provenance_roundtrip.rs` — test fixtures (2)
- `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs` — MCP test

### 3. NL ask-router (third, separate system)

`ask/patterns.rs` has 8 regex patterns that classify free-form English questions (`"what connects X to Y"`, `"who calls X"`) into `QuestionCategory` enums, then `dispatch.rs` routes to service methods. This is a **third query path** — disjoint from both MoldQL syntaxes. It does not produce MoldQL AST.

### ⚠️ Documentation vs Implementation Contradiction

Memory #3830 (2026-07-02) states: *"CONTEXT.md — added `MoldQL Syntax Levels` definition"* describing intent-first and graph-primitive levels. **Grep of CONTEXT.md confirms this definition does NOT exist.** Either the edit was lost, never committed, or the memory is aspirational. This must be resolved before proposal — the orchestrator should ask the user whether the "Syntax Levels" section was supposed to be committed.

## Affected Areas

- `crates/cognicode-explorer/src/moldql/parser.rs` — would need intent-grammar production rules or a pre-translation pass
- `crates/cognicode-explorer/src/moldql/ast.rs` — may need intent-level AST variants or a lowerer
- `crates/cognicode-explorer/assets/moldql-scaffolds.yaml` — templates currently unparseable; would become executable
- `CONTEXT.md` lines 265-269 — canonical examples; either parser must match or docs must change
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` — UI placeholders use intent syntax
- `crates/cognicode-explorer/src/ask/` — potential consolidation point if intent syntax absorbs NL routing

## Approaches

### 1. Lowerer / Pre-translation Layer
Add a `lower_intent_to_moldql(intent: &str) -> Result<MoldQLQuery>` function that translates lowercase English patterns (`symbols where ...`, `calls from X depth N`) into the existing uppercase AST. Parser stays unchanged.

- **Pros:** Zero risk to 32+ existing parser tests; scaffolds become executable immediately; incremental; reversible.
- **Cons:** Two grammars to maintain; translation rules can drift; doesn't help NL ask-router.
- **Effort:** Medium — ~200-400 lines of pattern-matching lowerer + tests.

### 2. Unified Parser with Dual Syntax
Extend the parser to accept both styles — lowercase intent (`symbols where ...`) and uppercase structured (`FIND symbols WHERE ...`) — producing the same AST.

- **Pros:** One parser, one AST; user-facing syntax matches docs; no translation layer.
- **Cons:** Parser ambiguity risk; existing error messages assume uppercase; larger blast radius.
- **Effort:** Medium-High — parser rewrite of leading-keyword dispatch + ambiguity resolution.

### 3. Intent DSL as First-Class AST (per memory #3830)
Add `MoldQLQuery::Intent(IntentQuery)` variants for goal-level queries (`vertical_slice`, `tests_covering`, `decision_trace`) that compile down to graph primitives. This is the full "two syntax levels" vision.

- **Pros:** Closest to the documented vision; scaffolds + NL router can both target it.
- **Cons:** Scope creep risk (full NLP system); needs design before implementation; memory #3830 is undocumented in CONTEXT.md.
- **Effort:** High — new AST layer + lowerer + spec work.

## Recommendation

**Approach 1 (Lowerer)** for v1. It makes the 9 production scaffold templates and all CONTEXT.md examples actually executable with the lowest risk. The lowerer is a pure function, fully testable, and reversible. Approach 3 is the right long-term target but needs a spec and CONTEXT.md alignment first — the memory/docs contradiction must be resolved before committing to it.

## Risks

- **Memory/docs contradiction**: Memory #3830 says a "Syntax Levels" section was added to CONTEXT.md but grep cannot find it. If the user believes it's documented, proposal will build on false ground.
- **Scope creep**: "Intent syntax" can easily expand into absorbing the NL ask-router or building a full NLP pipeline. v1 must stay bounded to making existing documented examples parse.
- **Scaffold execution gap**: Even with a lowerer, the ExplorerQL petgraph execution path returns empty results (`run_petgraph_plan` is a stub). Scaffolds using `calls from X depth N` will parse but return nothing until execution is wired.
- **Test fixture drift**: 9+ test files hardcode intent-syntax strings as expected values. If the lowerer changes what these produce, tests break.

## Ready for Proposal

**Yes — with one blocking question for the user.**

The orchestrator should ask:
> Memory #3830 says a "MoldQL Syntax Levels" section was added to CONTEXT.md describing intent-first and graph-primitive levels, but it's not in the file. Was that edit lost, or is the memory aspirational? This determines whether we're *aligning docs to code* or *building a documented vision*.

Once resolved, the proposal should scope to: (1) lowerer for the 3 documented intent patterns, (2) make scaffold templates executable, (3) align CONTEXT.md examples with reality. ExplorerQL execution wiring and NL-router consolidation are explicitly out of scope for v1.

---

## Envelope

- **status:** success
- **executive_summary:** MoldQL has a fully-implemented structured parser (7 uppercase keywords) but the lowercase "intent" syntax shown in CONTEXT.md, scaffold YAML, frontend, and 9+ test files is **not parseable** by the actual parser. A separate NL regex router exists as a third, disjoint path. The gap is concrete and bounded; a lowerer/translation layer is the lowest-risk v1 fix.
- **context_quality:** C1
- **taxonomy:**
  - dominant axes: `query_language_surface` (3 disjoint grammars), `docs_vs_impl_drift` (system-wide), `execution_completeness` (ExplorerQL compiles but stubs)
  - evidence: parser.rs, scaffold YAML, CONTEXT.md, 9+ grep-confirmed test files
- **artifacts:** this report
- **next_recommended:** resolve memory/docs contradiction → propose (Approach 1 lowerer)
- **risks:** memory contradiction blocking; scope creep into NLP; scaffold execution stub; test fixture drift
