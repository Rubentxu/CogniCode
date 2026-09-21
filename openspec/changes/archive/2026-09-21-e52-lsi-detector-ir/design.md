# Design — cycle e52 — Detector IR (M6.1)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15
> Change ID: `e52-lsi-detector-ir`

## Placement

New module `crates/cognicode-core/src/domain/findings/`, registered as
`pub mod findings;` in `domain/mod.rs`. **Ungated** (additive pure
domain), mirroring the M5 `domain::analytics::program_analysis`
precedent. No `sqlx`/`tokio`/I/O imports (domain purity rule).

```
domain/findings/
├── mod.rs           # re-exports
└── detector_ir.rs   # AnalysisLevel, DetectorStep, DetectorIr, authority, errors
```

## Type model

### `AnalysisLevel` (ordered escalation ladder)

```rust
pub enum AnalysisLevel {
    AstPattern,
    SemanticQuery,
    GraphQuery,
    Dataflow,
    AbstractInterpretation,
    Symbolic,
    RuntimeCorroboration,
    LlmContext,
}
```

- `rank() -> u8` — ascending cost (AstPattern = 0 … LlmContext = 7).
- `Display` — stable UPPER_SNAKE names (`AST_PATTERN`, …).
- `Ord` derived so "at least level L" is `>=`.

### IR nodes

```rust
pub struct SubjectPattern(String);   // validated "ns.name" namespace grammar
pub struct FindingKind(String);      // validated "ns.name"

pub enum DetectorStep {
    Match   { subject: SubjectPattern },
    Flow    { source: SubjectPattern, sink: SubjectPattern, max_hops: Option<u32> },
    Exclude { path_contains: SubjectPattern },
    Verify  { feasible_path: bool },
    Produce { kind: FindingKind },
}
```

`SubjectPattern`/`FindingKind` use a dot-separated namespace grammar
(`"ns.name"`, at least one `.`, no empty segments), validated on
construction. This matches the canonical detector-IR examples
(`security.user_input`).

### `DetectorIr`

```rust
pub struct DetectorIr {
    pub id: DetectorId,               // non-empty newtype
    pub name: String,
    pub required_level: AnalysisLevel,
    pub authority: DetectorAuthority,
    pub steps: Vec<DetectorStep>,
}
```

### `DetectorAuthority`

```rust
pub enum DetectorAuthority { Candidate, Gated }
impl DetectorAuthority { pub fn can_block(&self) -> bool { matches!(self, Self::Gated) } }
```

AI-authored detectors start as `Candidate` (umbrella REQ "AI detector
authority"); `can_block()` is the single gate predicate consumers use.

## Validation rules (`DetectorIr::validate`)

Fail loud with a structured `DetectorIrError` before admission (umbrella
REQ "Validated detector definition"):

| # | Rule | Error |
|---|------|-------|
| V1 | `id` non-empty | `EmptyId` |
| V2 | `name` non-empty | `EmptyName` |
| V3 | at least one `Produce` step | `NoProduce` |
| V4 | exactly one `Produce`, and it is the last step | `DuplicateProduce` / `ProduceNotLast` |
| V5 | every `Exclude`/`Verify` is preceded by a `Flow` | `StepOrder { index, step }` |
| V6 | every `Flow` source/sink has a preceding `Match` declaring the source, or is declared by the same `Flow` — simplified: a `Flow` must be preceded by at least one `Match` | `FlowWithoutMatch { index }` |
| V7 | declared `required_level` is **at least** the level the steps demand: `Flow` ⇒ ≥ `GraphQuery`; `Verify { feasible_path: true }` ⇒ ≥ `Symbolic` | `LevelTooLow { required, minimum }` |
| V8 | no `Match` after a `Flow` (patterns are declared first) | `StepOrder` |

`required` is the declared `required_level`; `minimum` the computed floor.
V7 implements "unsupported construct fails loud": a detector that asks to
`VERIFY feasible_path` while declaring `AST_PATTERN` is rejected at
registration rather than silently mis-scanning.

## Serde

All types derive `Serialize`/`Deserialize` with a stable
`#[serde(rename_all = "snake_case")]` (levels) / `rename` where needed.
Round-trip is tested for every variant (house pattern).

## Test surface

- `analysis_level_rank_is_strictly_ascending`
- `analysis_level_display`
- `subject_pattern_accepts_namespaced_and_rejects_malformed`
- `valid_graph_flow_detector_admits`
- `unsupported_construct_fails_loud` (V7)
- `flow_without_match_rejected` (V6)
- `exclude_before_flow_rejected` (V5)
- `no_produce_rejected` (V3)
- `produce_must_be_last` (V4)
- `candidate_detector_cannot_block` (authority)
- `detector_ir_round_trip` (serde)

## Rollback

Revert the two files + the one-line `domain/mod.rs` registration.

## Risks

- Namespaced grammar: the kernel `RelationKind` uses `"ns:name"` (colon)
  behind the `evidence-kernel` gate; detector subjects use `"ns.name"`
  (dot) matching the ANALYSIS-ENGINE examples. A tiny local validator is
  used here to avoid coupling M6 to the kernel feature gate. If the two
  ever need to converge, a shared `NamespacedName` can be extracted
  (ledger candidate).
