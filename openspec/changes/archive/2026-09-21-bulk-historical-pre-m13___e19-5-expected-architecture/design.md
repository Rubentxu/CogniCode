# Design: e19-5 Expected Architecture Boundary Rules

## Technical Approach

Extend E6 drift governance with dependency-boundary rules. Two additions to existing `#[cfg(feature = "multimodal")]` code: (1) `build_architecture_impl` infers `depends_on` edges between containers from workspace manifests; (2) `compare_architecture_impl` evaluates glob-pattern rules against those edges. Frontend gains additive overlay classes. All backward-compatible via `#[serde(default)]`.

## Architecture Decisions

### Decision: Dependency inference via two-pass Cargo.toml / package.json parse

**Choice**: Extend `build_architecture_impl` with a second pass: first collect `crate_name → container_id` from each member's `[package].name`, then re-iterate to emit `depends_on` edges for each `[dependencies]` entry matching a known internal crate. For `apps/*`, parse `package.json` `dependencies` against the same name map.
**Alternatives**: Parse `[workspace.dependencies]` path table only (rejected — misses `package.json` deps and non-workspace crate names). Persist inferred edges to PG (rejected — architecture view is a computed projection, not persisted graph data).
**Rationale**: Reuses the existing Cargo.toml parse loop already in `build_architecture_impl` (lines 350–417). Two-pass is required because the target container's id is not known until all containers are enumerated.

### Decision: Glob pattern matching via `glob` crate

**Choice**: Add `glob.workspace = true` to `cognicode-explorer` deps. Use `glob::Pattern::new(rule.from_pattern)` matching against the container path (stripped of `container:` prefix, e.g. `crates/cognicode-core`, `apps/explorer-ui`). Document that `*` does not cross `/`.
**Alternatives**: Regex (overkill for path-prefix rules). Custom wildcard matcher (reinvents the wheel). Match against basename only (loses directory context like `apps/*`).
**Rationale**: `glob` is already a workspace dependency. Pattern matching is path-aware — `apps/*` matches `apps/explorer-ui`, `crates/cognicode-*postgres*` matches `crates/cognicode-postgres`.

### Decision: `depends_on` edges as string-literal relations

**Choice**: Emit `GraphEdge { relation: "depends_on", style_class: "edge-depends-on" }`. Do NOT add `EdgeKind::DependsOn` to the core enum.
**Alternatives**: Add `EdgeKind::DependsOn` variant (broader core change, needs migrations, not needed for a display-only view).
**Rationale**: The architecture `SubgraphResponse` edges are display-oriented — `relation` is already a `String`. Existing code uses `EdgeKind::PartOf.as_str()` for part_of, but `depends_on` is a computed architectural relationship, not extracted graph data.

### Decision: `Severity` enum on `DependencyRule`, `String` stays on `DriftFinding`

**Choice**: New `Severity { Error, Warning, Info }` enum on `DependencyRule` (the config type). `DriftFinding.severity` remains `String` (existing E6 pattern). Convert `Severity` → `&str` at emit time.
**Alternatives**: Refactor `DriftFinding.severity` to `Severity` (breaking wire change for E6 consumers).
**Rationale**: Spec requires `error | warning | info` severity values. Typing the config side prevents invalid YAML. Keeping `DriftFinding.severity` as `String` preserves E6 backward compatibility.

### Decision: Frontend overlay on container nodes, independent toggle

**Choice**: Add three node overlay classes: `boundary-violation-error` (red border), `boundary-violation-warning` (amber), `boundary-violation-info` (blue). Both endpoints of a violating edge receive the border. Independent toggle from drift/hotspot overlays.
**Alternatives**: Color the violating edge instead of nodes (spec mandates node borders). Merge into existing drift toggle (spec mandates independence).
**Rationale**: Follows the established additive-overlay pattern (`border-color` + `border-width` only, never `background-color`).

## Data Flow

```
build_architecture_impl(root_path)
  │
  ├─ Pass 1: parse workspace Cargo.toml members + apps/* → collect containers
  │          build crate_name → container_id map
  │
  ├─ Pass 2: for each container, parse [dependencies] / package.json deps
  │          emit depends_on edge for each internal dep
  │
  └─ returns SubgraphResponse { nodes, edges (part_of + depends_on) }

compare_architecture_impl(root_path)
  │
  ├─ parse expected-architecture.yaml → ExpectedArchitecture { containers, dependency_rules }
  ├─ call build_architecture_impl → inferred graph
  ├─ E6 checks: missing / extra / wrong_sub_kind containers
  │
  └─ NEW: for each depends_on edge (src → dst):
         for each rule:
           if glob(rule.from, src_path) && glob(rule.to, dst_path):
             emit DriftFinding { kind: BoundaryViolation, severity, ... }
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modify | Add `Severity` enum, `DependencyRule` struct, `DriftKind::BoundaryViolation`, `dependency_rules` field on `ExpectedArchitecture`, `boundary_violations` field on `DriftReport` |
| `crates/cognicode-explorer/src/facades/graph.rs` | Modify | Two-pass `[dependencies]` inference in `build_architecture_impl`; glob-based rule evaluation in `compare_architecture_impl` |
| `crates/cognicode-explorer/Cargo.toml` | Modify | Add `glob.workspace = true` |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Modify | Add `boundary-violation-error/warning/info` overlay classes + entries in `KNOWN_NODE_CLASSES` |

## Interfaces / Contracts

```rust
// dto.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Error, Warning, Info }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRule {
    pub id: String,
    pub description: String,
    pub from_pattern: String,
    pub to_pattern: String,
    pub severity: Severity,
}

// ExpectedArchitecture gains:
#[serde(default)]
pub dependency_rules: Vec<DependencyRule>

// DriftKind gains:
BoundaryViolation,

// DriftReport gains:
#[serde(default)]
pub boundary_violations: usize,
```

```typescript
// stylesheet.ts — KNOWN_NODE_CLASSES gains:
"boundary-violation-error",
"boundary-violation-warning",
"boundary-violation-info",
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `glob::Pattern` matches expected container paths | Direct `Pattern::new().matches()` assertions |
| Unit | Two-pass inference emits `depends_on` edge for internal crate dep | `build_architecture_impl` with temp Cargo.toml workspace |
| Unit | Rule with `from_pattern` + `to_pattern` produces `BoundaryViolation` finding | `compare_architecture_impl` with temp `.cognicode/expected-architecture.yaml` |
| Unit | Missing `dependency_rules` → zero boundary violations, E6 unaffected | Existing E6 tests must pass unchanged |
| Integration | `package.json` dependency emits `depends_on` edge | `build_architecture_impl` with temp `apps/` dir |
| E2E | Violating container shows red border when overlay toggled on | Playwright (deferred — requires running UI) |

## Migration / Rollout

No migration required. `dependency_rules` defaults to empty `Vec` via `#[serde(default)]`; `boundary_violations` defaults to `0`. Existing `.cognicode/expected-architecture.yaml` files without the field produce identical E6 reports.

## Open Questions

- [ ] Should transitive boundary violations (A→B→C where A→C is forbidden) be detected in a post-MVP phase? Spec explicitly excludes this.
- [ ] Should `depends_on` edges appear in the default C4 graph render, or only when drift overlay is active?

## ADR Candidates

- **`depends_on` as string-literal relation (not `EdgeKind::DependsOn`)** — hard to reverse once persisted in PG; surprising that architecture edges bypass the typed enum; real trade-off between display flexibility and type safety. → ADR candidate.
