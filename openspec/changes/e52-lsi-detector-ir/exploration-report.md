# Exploration Report — cycle e52 — Detector IR (M6.1)

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: explore | Date: 2026-09-15

## Trigger

M0–M5 of the LSI program are archived. M6 "Findings & Detector IR" is the
next milestone. Its umbrella deliverables (ROADMAP M6):

- Finding/Evidence grades
- Detector IR
- legacy QualityIssue projection
- Axiom migration pipeline

Exit gates: AST + graph + dataflow detector examples; complete finding
causal explanation; QualityStore compatibility; false-positive benchmark.

This cycle targets the **first** bounded deliverable: the Detector IR
domain model (umbrella task 7.2), so later cycles (AST/graph/dataflow
backends, finding model, QualityIssue projection) have a stable contract
to compile against.

## Design inputs

| Source | Content |
|--------|---------|
| `docs/.../architecture/ANALYSIS-ENGINE.md` §Escalation engine | ordered analysis levels `AST_PATTERN → SEMANTIC_QUERY → GRAPH_QUERY → DATAFLOW → ABSTRACT_INTERPRETATION → SYMBOLIC → RUNTIME_CORROBORATION → LLM_CONTEXT`; policy escalates only when risk/ambiguity justifies |
| `ANALYSIS-ENGINE.md` §Detector IR | conceptual DSL: `MATCH source`, `FLOW source ->* sink`, `EXCLUDE path contains`, `VERIFY feasible_path`, `PRODUCE kind` |
| `ADR-AN-001-detector-ir-escalation` | IR compiles to AST/semantic/graph/dataflow/symbolic backends; AI may author only `CandidateDetector`, never a blocking detector |
| umbrella `specs/detector-ir/spec.md` | REQ "Validated detector definition" (parse/validate before admission; unsupported construct fails loud) + REQ "AI detector authority" (AI-authored detector starts as CandidateDetector without GATE authority) |
| `specs/finding-evidence-model/spec.md` | evidence grades A/B/C/D; gates may require a grade (consumed by the later finding cycle) |

## Landscape

| Path | Role |
|------|------|
| `crates/cognicode-core/src/domain/analytics/program_analysis/` | M5 precedent — additive domain module, **ungated**, `#![allow(missing_docs)]` |
| `crates/cognicode-core/src/domain/mod.rs` | module registration |
| `crates/cognicode-core/src/domain/services/` | existing detectors (`CycleDetector`) — algorithm-level, unrelated |
| `crates/cognicode-explorer/src/dto.rs` | legacy `FindingSeverity` (Info/Warning/Critical) + `DesignFinding` hypotheses |

There is **no existing** Finding or Detector-IR domain type in core; this
is greenfield.

## Strategy

Add a new ungated domain module `domain::findings` (mirroring the M5
`program_analysis` precedent) and land the Detector IR in this cycle:

- `AnalysisLevel` — the ordered escalation ladder with `rank()` + Display.
- `DetectorStep` — `Match`/`Flow`/`Exclude`/`Verify`/`Produce` IR nodes.
- `DetectorIr` — id, name, declared `required_level`, `authority`, steps.
- `DetectorAuthority` — `Candidate` (no GATE authority) vs `Gated`.
- `DetectorIr::validate()` — structural checks that fail loud with a
  structured `DetectorIrError` (mirrors REQ "unsupported construct fails
  loud" and "validated before admission").

The Finding/Evidence-grade model is deliberately deferred to the next
cycle so each slice stays within a reviewable size.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `domain/findings/` (new) | additive, pure domain, no I/O | KNOWN |
| `domain/mod.rs` | +1 module registration | KNOWN |
| default build | additive public types (mirrors M5) | KNOWN |
| other crates | none (no consumer yet) | LIKELY |

## Out of scope

- Finding/Evidence-class model (next cycle).
- Detector *backends* (AST/graph/dataflow execution).
- QualityIssue compatibility projection and Axiom import tooling.
- Wiring into MCP/CLI/Explorer.
