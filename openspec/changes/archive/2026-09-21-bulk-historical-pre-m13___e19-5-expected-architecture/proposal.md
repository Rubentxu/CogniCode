# Proposal: Expected Architecture Dependency Boundary Governance

## Intent

CogniCode's architecture drift detection currently compares only container **names** and **sub_kinds** against `.cognicode/expected-architecture.yaml`. It cannot detect when a container violates a dependency boundary — e.g., `explorer-ui` importing `postgres` crates directly, bypassing `cognicode-core`. This change extends drift governance from structural existence checks to **dependency boundary rules**, closing the gap between "the containers exist" and "they talk to the right things."

## Scope

### In Scope
- `dependency_rules` section in `expected-architecture.yaml` (deny-list patterns with severity)
- Container-to-container `depends_on` edge inference in `build_architecture_impl`
- `compare_architecture_impl` extension: rule evaluation → `BoundaryViolation` findings
- `DriftKind` + `DriftReport` DTO extension (`boundary_violations` counter)
- Frontend: `boundary-violation-error` / `-warning` CSS + `c4OverlaySlice` toggle

### Out of Scope
- PostgreSQL persistence of rules or drift history (YAML stays git-managed)
- CRUD API for rules (no admin endpoints)
- Allow-list / dependency-whitelist mode (deny-list only for MVP)
- Call-graph-based dependency inference (deferred — see Open Questions)

## Capabilities

### New Capabilities
- `architecture-drift-governance`: Expected-architecture YAML schema (containers + dependency_rules), container dependency inference via Cargo.toml, drift comparison producing boundary-violation findings.

### Modified Capabilities
None.

## Approach

**YAML schema**: Extend `ExpectedArchitecture` with optional `dependency_rules: Vec<DependencyRule>`. Each rule: `{ id, description, from_pattern, to_pattern, severity }`. Patterns glob-match container names.

**Dependency inference**: Extend `build_architecture_impl` to emit `depends_on` edges between containers. Source: workspace member `Cargo.toml` `[dependencies]` (already parsed; exclude `[dev-dependencies]` / `[build-dependencies]`). For `apps/*`: `package.json` dependencies. Deterministic, no graph ingestion needed.

**Comparison**: After existing name/sub_kind checks, evaluate each rule against inferred `depends_on` edges. A rule fires when an inferred edge matches both `from_pattern` and `to_pattern`, producing a `BoundaryViolation` finding with the rule's severity.

**Frontend**: Two new border-only Cytoscape classes (red=`error`, amber=`warning`) following the additive-overlay pattern from e19-3. New `boundaryViolationsEnabled` toggle in `c4OverlaySlice`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modified | `DriftKind::BoundaryViolation`, `DriftReport.boundary_violations`, `DependencyRule`, `ExpectedArchitecture.dependency_rules` |
| `crates/cognicode-explorer/src/facades/graph.rs` | Modified | `build_architecture_impl` emits `depends_on`; `compare_architecture_impl` evaluates rules |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Modified | `boundary-violation-error` / `-warning` classes |
| `apps/explorer-ui/src/state/slices/c4OverlaySlice.ts` | Modified | `boundaryViolationsEnabled` toggle |
| `.cognicode/expected-architecture.yaml` | New | Example file with `dependency_rules` |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Cargo.toml deps over-report (dev/build-deps) | Med | Filter `[dependencies]` only |
| Glob pattern ambiguity | Low | Document semantics; reuse existing crate globbing |
| Overlay conflicts with existing drift classes | Low | Separate toggle; additive classes per e19-3 |

## Rollback Plan

Remove `dependency_rules` from YAML (comparison silently skips — serde `default`). Revert `build_architecture_impl` to `PartOf`-only. DTO additions are backward-compatible (serde `default` on new fields).

## Dependencies
- e19-3-c4-overlays additive Cytoscape overlay pattern (border-only classes) — already designed

## Success Criteria
- [ ] A rule `from: "apps/*" to: "cognicode-*postgres*"` flags when `explorer-ui` depends on a postgres crate
- [ ] `DriftReport` includes `boundary_violations` count
- [ ] No `expected-architecture.yaml` → empty report (graceful degradation preserved)
- [ ] Frontend toggle renders boundary-violation borders on matched containers

## Open Questions
- **Dependency inference source**: Cargo.toml (static, simple, deterministic) vs symbol call graph (runtime, accurate, needs ingestion). MVP uses Cargo.toml; call-graph inference deferred. Trade-off: Cargo.toml misses dynamic coupling.
- Should `depends_on` inference cover transitive dependencies or only direct edges?
