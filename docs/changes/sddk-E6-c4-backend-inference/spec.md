# Kernel Specs: E6 C4 Backend Inference — Reconcile Roadmap + Fix Hardcoded System Name

## Router Context Used
- Knowledge Coverage: gapped (roadmap contradicts code; explore already verified end-to-end wiring)
- Context Quality: C2 (proposal + confirmed code locations + ADR-039 §8)
- Taxonomy: doc-drift + latent-bug (low-risk, mechanical)
- Domain Language: System (C1), Container (C2), Component (C3), Code (C4), `part_of` edge; `system_id` = `system:<lowercase_basename>`; `label` = `<lowercase_basename>`
- Recommended Effort: verify (single PR; no new crate; no type-safety refactor; no persistence)

## Knowledge Provenance
- Scope source: engram #2717 (Kernel Proposal: Reconcile E6 Roadmap + Fix Hardcoded System Name)
- Invariant source: ADR-039 §8 (`crates/cognicode-explorer/.../docs/adr/ADR-039-explorer-navigation-model.md:93`) — C4 inference uses heuristics from cognicode-core's existing features
- Code-truth source: `crates/cognicode-explorer/src/facades/graph.rs:201-417` (implementation) + `graph.rs:607-805` (6 tests)
- Roadmap source: `docs/explorer-roadmap.md:134-153` (Sprint E6)
- Memory-only hints excluded from spec truth: none

## Capability: C4 Backend Inference

### Requirement: REQ-E6-1 — System name derived from workspace root

The system SHALL derive the C1 system node ID and label from the basename of the workspace `root_path`, normalized to lowercase, so that any workspace — not just CogniCode's own repo — produces a correctly-labeled system node.

**Algorithm** (applied in `build_architecture`, `crates/cognicode-explorer/src/facades/graph.rs:201-216`):

```text
workspace_name = Path::new(root_path)
    .file_name()
    .map(|s| s.to_string_lossy().to_lowercase())
    .unwrap_or_else(|| "system".to_string())

system_id = format!("system:{}", workspace_name)
label     = workspace_name
```

The `let root = Path::new(root_path);` binding (currently at `graph.rs:222`) MUST be moved before the system-node construction block so the basename is available for both fields.

#### Scenario: REQ-E6-1.a — CogniCode's own repo preserves existing ID
**Given** `root_path` resolves to a directory whose basename is `"CogniCode"`
**When** `build_architecture` constructs the C1 system node (`graph.rs:201-216`)
**Then** the node `id` equals `"system:cognicode"` (current behavior preserved for the ID)
**And** the node `label` equals `"cognicode"` (lowercase normalization — a documented behavior change from the previous literal `"CogniCode"`)

#### Scenario: REQ-E6-1.b — Non-CogniCode workspace produces workspace-derived ID
**Given** `root_path` resolves to a directory whose basename is `"my-app"`
**When** `build_architecture` constructs the C1 system node
**Then** the node `id` equals `"system:my-app"`
**And** the node `label` equals `"my-app"`
**And** the `style_class` is `"node-system"` (unchanged)

#### Scenario: REQ-E6-1.c — Mixed-case basename is normalized
**Given** `root_path` resolves to a directory whose basename is `"My-Mixed-Case-App"`
**When** `build_architecture` constructs the C1 system node
**Then** the node `id` equals `"system:my-mixed-case-app"`
**And** the node `label` equals `"my-mixed-case-app"`

#### Scenario: REQ-E6-1.d — Empty / missing basename falls back
**Given** `root_path` is the empty string or otherwise yields no basename via `Path::file_name()`
**When** `build_architecture` constructs the C1 system node
**Then** the node `id` equals `"system:system"` (fallback value)
**And** the node `label` equals `"system"`

### Requirement: REQ-E6-2 — Existing 3 test assertions updated

The system SHALL update the three test assertions in `crates/cognicode-explorer/src/facades/graph.rs` that hardcode `"system:cognicode"` so they compute the expected ID from the temp-dir basename using the SAME algorithm as REQ-E6-1, keeping the suite honest about workspace-derived identity.

#### Scenario: REQ-E6-2.a — `build_architecture_returns_system_node` updated
**Given** the test at `graph.rs:607-622` runs with a `TempDir::new()` (whose basename is platform-random, e.g., `".tmpAbCdEf"`)
**When** the assertion at `graph.rs:620` runs
**Then** it MUST compute `expected = format!("system:{}", tmp.path().file_name().unwrap().to_string_lossy().to_lowercase())` and compare the system node `id` against `expected`
**And** the test passes regardless of the platform-generated basename

#### Scenario: REQ-E6-2.b — `build_architecture_includes_workspace_containers` updated
**Given** the test at `graph.rs:624-669` runs with `TempDir::new()`
**When** the edge filter at `graph.rs:665` runs (the `target == "system:cognicode"` literal)
**Then** it MUST compute `expected_system_id` from `tmp.path().file_name()` using the REQ-E6-1 algorithm
**And** the filter `e.target == expected_system_id` matches exactly one `part_of` edge
**And** the test passes

#### Scenario: REQ-E6-2.c — `build_architecture_creates_part_of_edges` updated
**Given** the test at `graph.rs:772-805` runs with `TempDir::new()`
**When** the assertion at `graph.rs:804` runs (the `targets.contains(&"system:cognicode")` literal)
**Then** it MUST compute `expected_system_id` from `tmp.path().file_name()` using the REQ-E6-1 algorithm
**And** the `targets` slice contains `expected_system_id` exactly once
**And** the test passes

### Requirement: REQ-E6-3 — New test for workspace-named system identity

The system SHALL provide one new `#[tokio::test]` in `crates/cognicode-explorer/src/facades/graph.rs` (added to the existing `mod tests` block at `graph.rs:590-805`) that asserts the system node is named after the workspace, not after CogniCode. The new test name MUST be `build_architecture_derives_system_name_from_workspace`.

#### Scenario: REQ-E6-3.a — New test creates a non-CogniCode temp dir
**Given** a new test at the end of the `mod tests` block in `graph.rs:805`
**When** the test creates a `tempfile::TempDir` using `tempfile::Builder::new().prefix("my-app").tempdir()` (basename starts with `"my-app."`)
**Then** `tmp.path().file_name()` yields a string beginning with `"my-app."` (lowercased)
**And** `build_architecture(tmp.path().to_str().unwrap())` returns successfully

#### Scenario: REQ-E6-3.b — New test asserts workspace-derived ID
**Given** the test setup from REQ-E6-3.a
**When** the test inspects the returned `SubgraphResponse.nodes`
**Then** it filters for `kind == "system"` and asserts exactly one match
**And** asserts `system_nodes[0].id == format!("system:{}", workspace_name)` where `workspace_name = tmp.path().file_name().unwrap().to_string_lossy().to_lowercase()`
**And** asserts `system_nodes[0].label == workspace_name`
**And** asserts `system_nodes[0].kind == "system"`
**And** asserts `system_nodes[0].style_class == "node-system"`

#### Scenario: REQ-E6-3.c — New test asserts the value is NOT CogniCode
**Given** the test setup from REQ-E6-3.a
**When** the test asserts the system node ID
**Then** the assertion `system_nodes[0].id != "system:cognicode"` MUST hold (proving the fix actually changes behavior for non-CogniCode workspaces)
**And** the assertion `system_nodes[0].label != "CogniCode"` MUST hold

### Requirement: REQ-E6-4 — Roadmap reconciliation

The system SHALL update `docs/explorer-roadmap.md` Sprint E6 (lines 134-153) so the table and status reflect the actual implementation state: C4 inference is shipped inside `GraphServiceImpl::build_architecture`; the `cognicode-diagram` crate is intentionally not extracted (YAGNI); `NodeKind`/`EdgeKind` typing is deferred.

#### Scenario: REQ-E6-4.a — Sprint E6 status header updated
**Given** the roadmap at `docs/explorer-roadmap.md:138`
**When** a reader opens Sprint E6
**Then** the status line reads: `"**Status:** ⚠️ Partial — inference complete; type-safety + crate extraction deferred"`

#### Scenario: REQ-E6-4.b — E6.1 marked Done with realistic evidence
**Given** the table row at `docs/explorer-roadmap.md:142`
**When** the table is read
**Then** row E6.1 status flips from `❌` to `✅`
**And** evidence column reads: `"Inference implemented in GraphServiceImpl::build_architecture (graph.rs:201-417); standalone `cognicode-diagram` crate not extracted (YAGNI — single consumer)"`

#### Scenario: REQ-E6-4.c — E6.2, E6.3, E6.4 marked Done
**Given** rows E6.2, E6.3, E6.4 at `docs/explorer-roadmap.md:143-145`
**When** the table is read
**Then**:
- E6.2 status `✅` Done — evidence: `"Cargo.toml members + package.json parsing in graph.rs:228-290 (C2 containers)"`
- E6.3 status `✅` Done — evidence: `"Symbol cap 200 in graph.rs (C3 components + C4 code)"`
- E6.4 status `✅` Done — evidence: `"build_architecture() returns SubgraphResponse (used by `GET /api/workspaces/:id/architecture`) — graph.rs:201"`

#### Scenario: REQ-E6-4.d — E6.5 marked Partial
**Given** row E6.5 at `docs/explorer-roadmap.md:146`
**When** the table is read
**Then** row E6.5 status flips from `❌` to `⚠️ Partial`
**And** evidence column reads: `"Kinds are string-typed (`"system"`, `"container"`, `"component"`, `"code"`) — typed `NodeKind`/`EdgeKind` deferred to future hardening; see engram #2717"`

### Requirement: REQ-E6-5 — Verification

The change SHALL pass the existing test suite plus the new test, compile cleanly across the workspace, and produce a roadmap that matches the code.

#### Scenario: REQ-E6-5.a — `cargo test -p cognicode-explorer` passes
**Given** the patch is applied on top of `main @ 2b5dac4`
**When** `cargo test -p cognicode-explorer` is run
**Then** all 6 previously-passing tests still pass (with REQ-E6-2 assertions updated)
**And** the 1 new test from REQ-E6-3 passes
**And** the command exits 0

#### Scenario: REQ-E6-5.b — `cargo check --workspace` exits 0
**Given** the patch is applied
**When** `cargo check --workspace` is run
**Then** the command exits 0
**And** no new warnings or errors are introduced (the algorithm uses `Path::file_name()` which is already in scope at `graph.rs:222`, so no new imports are needed; the `let root = Path::new(root_path)` binding is moved but not duplicated)

#### Scenario: REQ-E6-5.c — Roadmap reflects code-truth
**Given** the patch is applied
**When** `rg 'Sprint E6' docs/explorer-roadmap.md` is run
**Then** the matched section contains no `❌` rows for E6.1-E6.4
**And** E6.5 row reads `⚠️ Partial`
**And** the status header reads the new text from REQ-E6-4.a

## Invariants Covered
- **ADR-039 §8 heuristic contract**: Cargo.toml/package.json → containers, dirs → components, symbols → code (cap 200) — preserved; only the C1 system-node identity changes. Verification: existing tests at `graph.rs:624-769` still pass unchanged (containers, apps, components, cap-200).
- **Algorithm determinism**: same `root_path` always yields the same `system_id` and `label` — verified by tests REQ-E6-2.a/b/c computing expected from the temp dir basename using the exact production algorithm.
- **No new crate**: no `Cargo.toml` change. Verified by `cargo check --workspace` (REQ-E6-5.b) and absence of any new crate directory under `crates/`.
- **No persistence**: only `build_architecture` in-memory behavior changes. No DB schema touched.
- **CogniCode self-hosting preserved**: `system_id` remains `"system:cognicode"` for the project's own repo (REQ-E6-1.a) — the only ID behavior that matches today exactly.

## Out of Scope
- `cognicode-diagram` crate extraction (YAGNI — single-method, single-consumer)
- Type-safety refactor with typed `NodeKind`/`EdgeKind` enums (Option B — deferred to future hardening; tracked in engram #2717)
- C4 node persistence (no consumer today)
- IaC / Python / Go language extraction
- Frontend label-case customization for CogniCode's own repo (the label becomes lowercase `"cognicode"`; this is the documented consequence of the `to_lowercase()` algorithm and is NOT in scope to special-case)
- Snapshots / golden tests of C4 visualization output
- ADR-039 amendments (the §8 heuristic contract is preserved as-is)

## Verification Commands
```bash
# All tests pass (6 updated + 1 new = 7 total in cognicode-explorer)
cargo test -p cognicode-explorer

# Workspace compiles cleanly
cargo check --workspace

# No new crate introduced
ls crates/ | wc -l   # count must equal pre-patch value

# Roadmap reflects truth
rg 'Sprint E6|E6\.[1-5]' docs/explorer-roadmap.md

# Production code references REQ-E6-1 algorithm (smoke check)
rg 'system:' crates/cognicode-explorer/src/facades/graph.rs
```

## Open Questions
- None blocking. The CogniCode label change from `"CogniCode"` → `"cognicode"` is the documented consequence of the `to_lowercase()` algorithm (REQ-E6-1.a); if product/branding wants the original case preserved for the project's own repo, that is a follow-up decision (e.g., a separate `display_name` field, or a special case for `basename == "CogniCode"`). Not blocking this change.
