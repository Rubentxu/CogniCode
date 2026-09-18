# DEBT-3 — Adopt identity taxonomy for distribution

> Status: **PASS** (taxonomy adopted, heuristics classified, invariants pinned)
> Closure date: 2026-09-18
> Apply commits: `c7fb5cb9` (WU0+WU1+WU2+WU4) · `959287c5` (WU3 audit) · `432713b8` (WU5 invariants)
> Initiative: `arch-identity-taxonomy`

## Origin

The UAT Phase D `link_or_copy failed` bug surfaced a deeper
problem: across the distribution chain, four different
identities — `PluginId`, `ComponentId`, `BinaryName`,
`SkillBundleId` — were sharing strings by coincidence rather
than by declaration. The same literal `cognicode-mcp` was
simultaneously:

* a `BinaryName` (the shim that lands at `<root>/shims/cognicode-mcp`),
* a `ComponentId` (the bundle's `DaemonCli` component stem),
* an MCP config key (`mcp.cognicode-mcp`),
* a `SkillBundleId` (`skills/cognicode-mcp/`).

Worse: the live `mcp-server` plugin (a `PluginId`) was
implicitly assumed by the IDE adapters to be the same as the
bundled `cognicode-mcp` DaemonCli (a `ComponentId`). The
installer resolved them as the same string. The legacy
install path even derived the component directory name from
the plugin name, baking the confusion into the filesystem
layout.

This cycle codifies the separation.

## What changed

### WU0 — Inventory (in `c7fb5cb9`)

* 5 distinct identity types enumerated: `PluginId`,
  `ComponentId`, `BinaryName`, `ArtifactKind`, `SkillBundleId`.
* Source-of-truth mapping per identity:
  - `PluginId` → `crates/cognicode-cli/src/cmd/bundled/<name>.yaml` (plugin manifest).
  - `ComponentId` → bundle manifest's `BundleComponent.name`; must equal `kind.stem()`.
  - `BinaryName` → the published file inside the component directory.
  - `ArtifactKind` → enum from the bundle manifest; not a string.
  - `SkillBundleId` → `crates/cognicode-cli/src/cmd/skill.rs::SkillManifest.name`.

### WU1 — Characterize (in `c7fb5cb9`)

* Worked example for `mcp-server`: `PluginId = "mcp-server"`,
  `ComponentId = "cognicode-mcp"`, `BinaryName = "cognicode-mcp"`.
* Worked example for `skills-cognicode-core`:
  `PluginId = "skills-cognicode-core"`, declares zero components
  (skill-only plugin), no `BinaryName` produced.
* Worked example for the IDE adapters (`zcode`, `claude`,
  `codex`, `opencode`): each is a `PluginId` whose `binaries[]`
  declares the IDE adapter binary name. The adapter does not
  produce a `SkillBundleId` and does not own a `ComponentId`.

### WU2 — ADR (in `c7fb5cb9`)

* New file: `docs/adr/ADR-IDENTITY-MAP-distribution.md` (566
  lines, force-added via `git add -f`). Status: **proposed**.
  Decision: every identity carries an explicit declaration in
  a manifest; no identity may be derived from another just
  because they happen to share a string.

### WU3 — Inference audit (in `959287c5`)

* Appended §9-§11 to the ADR. Classification of every
  stringly-typed site in production code:

  | File:line | Heuristic | Disposition |
  |---|---|---|
  | `installer_transaction.rs:423` | `bin/<comp>/<comp>` Cargo-style path derived from `bin.name` | **Eliminate** (derive from `ComponentId` via `home.component_root`) |
  | `install.rs:67` | "first directory under `skills_root`" lookup | **Parked** (DEBT-2 needs portable-skill-bundle manifest modelling first) |
  | `ide.rs:730` | literal `"cognicode-mcp"` hardcoded for the MCP config key | **Eliminate** (read `BinaryName` from the bundle's DaemonCli component) |
  | `ide.rs:245,266` | `vec!["mcp", "cognicode-mcp"]` for opencode JSON merge | **Eliminate** (replace with `home.shim_path(binary_name)` join) |
  | `ide.rs:397,422` | literal `"zcode"` and `"claude"` paths | **Eliminate** (read from `manifest.binaries[]`) |
  | `ide.rs:494,515` | `.claude/mcp/cognicode-mcp.json` literal | **Eliminate** (derive from BinaryName) |
  | `ide.rs:631,665` | `codex.toml` literal | **Eliminate** (derive from `ide.id`) |
  | `ide.rs:737` + `:367-374, :477-481, :593-597` | PluginId-as-skills-dir path joins | **Eliminate** (use `home.skills_root(v)`) |

* Test fixtures: 55 occurrences of `"mcp-server"` literal in
  `tests:` blocks (ADR §9.7) — explicitly accepted; only
  production code is in scope.

### WU4 — Compatibility decision (in `c7fb5cb9`)

* `mcp-server` (PluginId) — **verdict A**: valid PluginId,
  no action needed. The bundle correctly resolves its
  ComponentId via the plugin manifest.
* `cognicode-core` (SkillBundleId, archived skill bundle) —
  **verdict B**: legacy identifier, retire on next skill
  catalog refresh.
* `cognicode-mcp-driven` (SkillBundleId, archived) —
  **verdict B**: legacy identifier, retire on next skill
  catalog refresh.
* `cognicode` (SkillBundleId AND ComponentId, both live) —
  **verdict D**: intentionally shared across two namespaces.
  ADR §6.4 documents why this is acceptable (the two
  namespaces live under orthogonal filesystem subtrees;
  filesystem location is the disambiguator, not the string).

### WU5 — Invariant tests (in `432713b8`)

* `crates/cognicode-cli/src/cmd/layout.rs` — three new tests,
  all green on first run:
  - `t_debt3_component_root_disjoint_from_skill_bundle` (T2):
    Distinct identities produce disjoint paths; the shared
    `cognicode` identity STILL resolves to disjoint paths
    because filesystem location is the disambiguator.
  - `t_debt3_plugin_id_not_derivable_from_component_id` (T3):
    The live `mcp-server` plugin and the bundled
    `cognicode-mcp` DaemonCli produce disjoint paths under
    orthogonal home subtrees (`plugins/` vs `version_root(<v>)/`).
    No silent cross-derivation.
  - `t_debt3_adversarial_three_distinct_identities` (T4):
    Headline adversarial test. Four distinct identities
    (PluginId, ComponentId, BinaryName, SkillBundleId) resolve
    to four pairwise-disjoint canonical paths. The flow is
    identity-driven, not name-driven.

* No T1 ("zero identity-bridging literals in non-test
  production source"). A strict T1 would fail today because
  the heuristics catalogued in ADR §9.4 are still present in
  `ide.rs` and `install.rs`. Eliminating those literals is
  **DEBT-3.f's work** and will ship its own RED-GREEN cycle
  with a strict T1 as the regression gate. Until then, the
  ADR audit table is the human-checked source of truth.

## Verification

* `cargo test -p cognicode-cli --bins t_debt3`:
  **3 passed / 0 failed**. T2, T3, T4 all green.
* `cargo test -p cognicode-cli --bins layout::`:
  **31 passed / 0 failed**. No regression in the broader
  layout suite.
* `cargo check -p cognicode-cli --tests`:
  finished, no errors. (39 warnings, all pre-existing.)
* `cargo fmt --check`: clean.

## What did NOT change

* The 5 canonical layout helpers
  (`version_root`, `component_root`, `version_manifest`,
  `skills_root`, `skill_bundle`) introduced in L1 of the
  predecessor cycle `arch-canonical-layout` — unchanged.
* The bundle manifest format
  (`crates/cognicode-cli/src/cmd/bundle_manifest.rs`) —
  unchanged. The cycle's Stop condition (no public format
  change) is honoured.
* The plugin manifest format
  (`crates/cognicode-cli/src/cmd/manifest.rs::PluginManifest`)
  — unchanged.
* The skill manifest format
  (`crates/cognicode-cli/src/cmd/skill.rs::SkillManifest`) —
  unchanged.
* Production IDE-adapter code (`ide.rs`, `install.rs`) —
  unchanged. The heuristics are catalogued and elimination
  is scheduled as DEBT-3.f, not silently landed.
* Any HEURISTIC literal in production code — unchanged.
  The cycle's contract is "audit and pin", not "audit and
  silently fix". The audit table is the deliverable; the
  elimination cycles are follow-ups.

## Cycle verdict

**PASS**. Bounded, contract-respecting cycle:

* 3 apply commits (c7fb5cb9, 959287c5, 432713b8).
* 1 new ADR (force-added, 566 lines).
* 1 new test module section (`+168` lines, single file).
* 0 production source changes (Stop condition honoured).
* 0 public format changes (Stop condition honoured).

The architectural cycle `arch-identity-taxonomy` is now
closed. The contradiction between "an identity is whatever
string happens to be in the path" and "an identity is
declared in a manifest" is resolved at the model level;
the elimination of every heuristic that worked around the
contradiction is filed as a follow-up cycle so each
heuristic gets its own bounded RED-GREEN.

## Follow-ups filed separately (NOT part of this archive)

* **DEBT-3.f — Eliminate identity-bridging heuristics**: ship
  the strict T1 gate test (`zero identity-bridging literals
  in non-test production source`), and replace each heuristic
  site catalogued in ADR §9.4 with a typed helper call. Bounded
  RED-GREEN per heuristic. The first site to eliminate is
  `installer_transaction.rs:423` (Cargo-style `bin/<comp>/<comp>`
  derivation) — the simplest and most isolated.
* **DEBT-2 — Portable-skill-bundle manifest modelling**: the
  bundle manifest models runtime components (cogh,
  cognicode-mcp) but not portable skill bundles (cognicode-core,
  cognicode-mcp-driven). The `install.rs:67` "first directory
  under skills_root" heuristic remains the fallback until
  ArtifactKind gains a `SkillBundle` variant (or a parallel
  skill bundle manifest format). Opening **after DEBT-3.f**
  closes, so SkillBundleId attaches to a stabilised
  ComponentId.
* **DEBT-1 — HOME config sha256 drift** (`opencode.json`):
  pre-existing, not DEBT-3-specific. Filed separately.
* **DEBT-4 — Journal retention policy**: not addressed by
  DEBT-3, filed separately.
