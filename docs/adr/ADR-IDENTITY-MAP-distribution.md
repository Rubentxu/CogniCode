# Identity map — distribution identity and plugin-vs-component naming

> Companion to the architectural decision on which strings deserve to be
> first-class identities (`PluginId`, `ComponentId`, `ArtifactKind`,
> `BinaryName`, `SkillBundleId`) and which are derivable from which.
>
> Produced during the architectural cycle `arch-identity-taxonomy`
> (DEBT-3 per `openspec/changes/archive/2026-09-18-arch-l5-.../proposal.md`).
>
> ADR status: **proposed** (WU2 of the cycle; will move to **accepted** after
> WU3 + WU4 + WU5 land).

## 1. Why this ADR exists

Naming across the distribution chain has drifted. Five distinct concepts
share the same identifier in some places and diverge in others:

```text
mcp-server                 ← a PluginId (CLI argument + on-disk plugin dir name)
cognicode-mcp              ← a BinaryName (shim name + MCP config key) AND a
                             SkillBundleId (live skill bundles in skills/)
cognicode                  ← a ComponentId (BundleComponent.name in bundle.yaml)
                             AND a SkillBundleId (live skill bundles in skills/)
cognicode-core             ← a SkillBundleId (pre-productization, archived)
cognicode-mcp-driven       ← a SkillBundleId (pre-productization, archived)
```

The drift already produced a UAT-visible regression. In `cmd/install.rs`
(line 46–49 pre-L4), the IDE integration's `skill_path` was derived as
`<root>/install/<v>/mcp-server/skills`. The assumption was that
`PluginId == SkillBundle` and `PluginId == BinaryName`. Both are false:

- `mcp-server` (plugin name) does not name any on-disk skill bundle. The
  live skill bundles are named `cognicode` and `cognicode-mcp`.
- `mcp-server` does not name any binary. The MCP daemon binary is named
  `cognicode-mcp`. Pre-productization the per-bundle artifacts were
  `cognicode-core-*.tar.gz` and `cognicode-mcp-driven-*.tar.gz`.

This ADR documents which identities exist, where each one is declared,
which derivations are legitimate, and which are heuristics that need to
be replaced before any further work on the IDE adapter surface or the
bundle manifest format.

## 2. Scope and out-of-scope

**In scope.** Names that participate in the distribution chain:
plugin manifests, bundle manifests, portable skill bundles, artifacts,
binaries, shims, IDE integration config keys. Each is enumerated,
its current source-of-truth is named, and its consumers are listed.

**Out of scope (separate cycles).**

- `e84.1` / DEBT-2: portable-skill-bundle manifest modelling. This ADR
  pins `SkillBundleId` as a name; the manifest format that would let
  plugins reference specific bundles in a richer way is DEBT-2.
- DEBT-1: `write_json_atomic` re-serialisation drift.
- DEBT-4: journal lifecycle retention policy.

## 3. Five identities, ten declarations

Each identity has a role, a source of truth, and a list of consumers.
"Producer" means the entity that introduces the identity (a manifest, a
constant, a derivation rule). "Consumer" means the entity that reads or
uses the identity in a downstream computation.

### 3.1 `PluginId`

A `PluginId` is the user-facing identifier the CLI accepts as the
argument to `cogh <install|uninstall|update|latest|...> <plugin>` and as
the directory name under `~/.cognicode/plugins/`.

| Property | Value |
|---|---|
| Role | CLI argument, lock-file key, on-disk plugin manifest directory name. |
| Producer | `bundled/mcp-server.yaml` (`name: mcp-server`); `bundled/skills-cognicode-core.yaml` (`name: skills-cognicode-core`); `bundled/sandbox-templates.yaml` (`name: sandbox-templates`); `bundled/zcode.yaml`, `claude.yaml`, `codex.yaml` (each `name: <ide>`); `plugin.yaml` files registered via `cogh plugin add`. |
| Consumer | `layout.rs::cmd_install` (`home.plugin(plugin)`); `layout.rs::cmd_uninstall`; `ide.rs::cmd_ide_install` (passed straight through to `integrate_<ide>`); `cogh install <plugin> --ide <ide>` in `bin/cogh.rs`; `lockfile.rs` (`lf.pin("mcp-server", "0.92.0")`). |
| Filesystem ownership | `~/.cognicode/plugins/<PluginId>/plugin.yaml`. |
| User-visible? | Yes — appears in CLI commands, output, lock file. |
| Versioned? | No (the plugin itself has versions, but the PluginId is stable across versions). |
| Validation | `PluginManifest::validate` requires non-empty `name`, but does NOT enforce any constraint about matching the binary or component names. |

**Source of truth (production):** `bundled/mcp-server.yaml` (and the
five other bundled manifests) for the first-party set;
`<root>/plugins/<name>/plugin.yaml` for third-party plugins registered
via `cogh plugin add`.

**In the spec layer:** `openspec/specs/cognicode-plugin/spec.md` shows
the format and lists 4 bundled plugins (`mcp-server`,
`skills-cognicode-core`, `sandbox-templates`, `opencode`). Only the
first 3 are currently bundled in source; `opencode` is referenced
elsewhere but its `bundled/opencode.yaml` is absent. That mismatch
itself is a DEBT-3 candidate.

### 3.2 `ComponentId`

A `ComponentId` is the canonical identifier of an installable runtime
component within a bundle. Today it is *equal* to `BinaryName` and to
the artifact filename stem; the bundle's compile-time invariant
("`name` must equal `kind.stem()`") makes that bridge explicit.

| Property | Value |
|---|---|
| Role | Identifier of a `BundleComponent` in a `BundleManifest`. |
| Producer | `crates/cognicode-cli/src/cmd/bundle_manifest.rs` (`BundleComponent.name`); `crates/cognicode-cli/src/cmd/release_contract.rs::COMPONENTS` (the product surface table). |
| Consumer | `installer_transaction.rs:391,398,423` (extracts and shim lookups all use `comp.name`); `layout.rs::version_root/component_root/version_manifest`; `dev-bundle.yaml` profile tests. |
| Filesystem ownership | `<root>/versions/<v>/<ComponentId>/...`. |
| User-visible? | Indirectly — appears as the per-component directory name and as the binary inside the bundle. The user does not type the component name directly; they type the PluginId and the profile. |
| Versioned? | Yes — co-versioned with the bundle (bundle version = component version). |
| Validation | `BundleManifest` validates `name` is non-empty and equal to `kind.stem()`. |

**Source of truth (production):** `release_contract.rs::COMPONENTS`.
Each entry's `kind.stem()` is the canonical ComponentId:

| `ArtifactKind` | stem (= ComponentId) | published? | profile |
|---|---|---|---|
| `Cogh` | `cogh` | yes (Layer 0 boot) | — |
| `Cognicode` | `cognicode` | yes | core, reviewer |
| `DaemonCli` | `cognicode-mcp` | yes | reviewer |
| `ExplorerApi` | `explorer-api` | no | reviewer |

**Other ComponentIds not in `COMPONENTS`:** the `Profile` test fixture
(`profile.rs`) hardcodes a dev bundle with `cognicode` and `cognicode-mcp`
which match the table.

### 3.3 `BinaryName`

`BinaryName` is the basename of an executable produced by extracting a
component bundle. It is also the name of the symlink created under
`~/.cognicode/shims/` and the JSON/TOML merge path used in IDE
config files (`mcp.<BinaryName>`).

| Property | Value |
|---|---|
| Role | Filename of the executable inside `<root>/versions/<v>/<ComponentId>/bin/<BinaryName>`; symlink name under `<root>/shims/<BinaryName>`; JSON merge key under `mcp.<BinaryName>` in `opencode.json` / `zcode/config.json`. |
| Producer (planned) | `PluginManifest.binaries[].name` (e.g. `cognicode-mcp` in `bundled/mcp-server.yaml`). |
| Producer (actual today) | `install.rs` and `ide.rs` hardcode the literal `cognicode-mcp`. The bundle manifest's `comp.name == kind.stem()` invariant makes this consistent with `ComponentId` *for the bundle world* — but the plugin world's `binaries[]` is a separate declaration that the IDE adapter currently does not consume. |
| Consumer | `install.rs::run_install` (`home.shim_path("cognicode-mcp")`); `ide.rs::cmd_ide_install` (`home.shims().join("cognicode-mcp")`); `ide.rs::integrate_opencode/zcode/claude/codex` (`mcp.cognicode-mcp` JSON merge path). 5 references in `bundled/*.yaml` (`merge_path`, `command`, etc.). |
| Filesystem ownership | `<root>/versions/<v>/<ComponentId>/bin/<BinaryName>` (extracted) and `<root>/shims/<BinaryName>` (symlink). |
| User-visible? | Yes — the user types `cognicode-mcp` (or whatever the shim name is) on their shell. |
| Versioned? | No (the binary lives under a versioned directory but the name is stable). |
| Validation | `PluginManifest::validate` checks sha256 format but not the binary naming relationship. |

### 3.4 `ArtifactKind`

`ArtifactKind` is the discriminator in `BundleManifest.components[].kind`
and the type tag in `COMPONENTS`. It is the *only* identity currently
typed as a Rust enum, and it determines the artifact filename stem
(`kind.stem()`).

| Property | Value |
|---|---|
| Role | Type tag; decision input for which kinds are installable (`is_installable()`), which are Layer 0 vs Layer 1 vs Meta. |
| Producer | `release_contract.rs::ArtifactKind` enum (closed set: `Cogh`, `Cognicode`, `DaemonCli`, `ExplorerApi`, `BundleManifest`, `ReleaseInventory`, `Checksums`). |
| Consumer | `release_contract.rs::COMPONENTS` (the product surface table); `bundle_manifest.rs::BundleComponent.kind`; `release_factory.rs` (`reject_layer0_and_meta_kinds_in_a_bundle`, etc.). |
| Filesystem ownership | None directly; the kind drives the artifact filename stem and the install rules. |
| User-visible? | Indirectly — the kind is encoded into the artifact filename and the bundle validation rejects phantom kinds. |
| Versioned? | No (the enum is closed and stable). |
| Validation | `is_installable()`; `layer()` Layer classifier; bundle refuses Layer 0 + Meta in Layer 1 manifest. |

**Closed set today; do not add `SkillBundle` here without an ADR.**
`ArtifactKind` deliberately does not model skill bundles. The skill
bundles are a parallel concept owned by `SkillBundleId` and
`SkillManifest`. That separation is intentional and is the contract
`openspec/specs/portable-skill-bundle/spec.md` already declares.

### 3.5 `SkillBundleId`

`SkillBundleId` is the directory name of a portable skill bundle under
`~/.cognicode/versions/<v>/skills/<SkillBundleId>/` and the `name` field
of `SkillManifest`. It declares its dependency on plugins via
`requires: Vec<PluginId>`.

| Property | Value |
|---|---|
| Role | Per-skill identity; the directory under `<root>/versions/<v>/skills/`; the `name` of `SkillManifest`. |
| Producer | `bundled/skills/*.yaml` (`skills/cognicode/manifest.yaml` → `name: cognicode`; `skills/cognicode-mcp/manifest.yaml` → `name: cognicode-mcp`); `bundled/skills-cognicode-core.yaml` (`name: skills-cognicode-core` — plugin wrapper); archived bundles `cognicode-core`, `cognicode-mcp-driven`. |
| Consumer | `install.rs::run_install` ("first directory under `skills_root`" heuristic); `ide.rs::cmd_ide_install` (`integrate_opencode(skill_path, ...)` — `skill_path` is `versions/<v>/<plugin>/skills`, but the actual skill directory is `versions/<v>/skills/<bundle>/`); `cmd_ide_install` zcode/claude/codex branches (same shape). |
| Filesystem ownership | `<root>/versions/<v>/skills/<SkillBundleId>/`. |
| User-visible? | Yes — the user navigates `~/.cognicode/versions/<v>/skills/cognicode-mcp/`. |
| Versioned? | Yes — every skill carries its own `version` field; bundles are co-versioned with the runtime version per spec ("Skill bundles are versioned with the CogniCode version"). |
| Validation | `SkillManifest::validate` checks name non-empty, maturity ∈ {experimental, beta, stable, deprecated}. Does not check `requires` against installed plugins. |

## 4. Worked example (WU1)

Using the live published state as of HEAD `e4985597` (post-arch-canonical-layout
L5 archive) plus the live source tree:

### 4.1 The `mcp-server` plugin

| Concept | Identity | Source of truth |
|---|---|---|
| PluginId | `mcp-server` | `bundled/mcp-server.yaml::name` |
| PluginArtifact filename stem | `cognicode-mcp` (= BinaryName = ComponentId) | `bundled/mcp-server.yaml::versions[].artifact` |
| BinaryName | `cognicode-mcp` | hardcoded in `install.rs:67`, `ide.rs:730`, `bundled/zcode.yaml:25`, `bundled/claude.yaml:24`, `bundled/codex.yaml:24` |
| ComponentId | `cognicode-mcp` | `release_contract.rs::COMPONENTS[DaemonCli].kind.stem()` |
| ArtifactKind | `DaemonCli` | `release_contract.rs::ArtifactKind::DaemonCli` |
| MCP config key | `mcp.cognicode-mcp` | `bundled/zcode.yaml::merge_path`, `ide.rs:245`, `ide.rs:266` |
| Skill bundles inside this plugin's install? | None (the bundle manifest only declares runtime components; portable skill bundles are external). |

This is the **only** case in the codebase where the PluginId and the
ComponentId/BinaryName differ by design. It is documented and intentional
in the spec — `mcp-server` is the legacy plugin name from the asdf-style
plugin era, and `cognicode-mcp` is the stem of the daemon that the
plugin ultimately installs. The plugin world predates the bundle world.

### 4.2 The `skills-cognicode-core` plugin

| Concept | Identity | Source of truth |
|---|---|---|
| PluginId | `skills-cognicode-core` | `bundled/skills-cognicode-core.yaml::name` |
| PluginArtifact filename stem | `skills-cognicode-core-0.92.0.tar.gz` (= PluginId as stem) | `bundled/skills-cognicode-core.yaml::versions[].artifact` |
| BinaryName | none (`binaries: []`) | `bundled/skills-cognicode-core.yaml::binaries` |
| ComponentId | none (not a bundle-manifest artifact) | — |
| Skill bundles inside this plugin's install? | Whatever was inside the tarball — historically `cognicode-core/` and `cognicode-mcp-driven/` (now archived). |

This plugin's lifecycle is the source of DEBT-2 (portable-skill-bundle
manifest modelling). The plugin is a *delivery vehicle* for skill
bundles. The bundles themselves have their own identity
(`SkillBundleId`).

### 4.3 The `cognicode` and `cognicode-mcp` skill bundles

| Concept | Identity | Source of truth |
|---|---|---|
| SkillBundleId | `cognicode`, `cognicode-mcp` | `skills/cognicode/manifest.yaml::name`, `skills/cognicode-mcp/manifest.yaml::name` |
| Plugin that the skill requires | `mcp-server` (only for the MCP variant) | `skills/cognicode-mcp/manifest.yaml::requires` |
| Filesystem ownership | `<root>/versions/<v>/skills/cognicode*/` |
| ComponentId overlap | Both happen to share names with bundle components: `cognicode` matches `release_contract.rs::COMPONENTS[Cognicode].stem()` and `cognicode-mcp` matches `COMPONENTS[DaemonCli].stem()`. This is coincidence-of-naming, not a declared relationship. |

Here lies the trap. Because `SkillBundleId = cognicode-mcp` and
`BinaryName = cognicode-mcp` and the MCP config key is `mcp.cognicode-mcp`,
anyone reading the code can fall into the assumption that the skill
bundle is *part of the bundle component*. It is not. `cognicode-mcp`
the component is the daemon binary; `cognicode-mcp` the skill bundle is
the IDE-side workflow guide for that daemon. Two different things that
happen to share a name.

The two concepts are linked through the skill manifest's `requires:
[mcp-server]`, which uses the PluginId (not the ComponentId, not the
BinaryName) — exactly because the author wanted to encode the
plugin-level dependency.

### 4.4 The IDE adapters (`zcode`, `claude`, `codex`, `opencode`)

| Concept | Identity | Source of truth |
|---|---|---|
| PluginId (= IdeAdapterId) | `zcode`, `claude`, `codex`, `opencode` | `bundled/*.yaml::name` (`kind: IdeAdapter`) |
| MCP merge path JSON key | `mcp.cognicode-mcp` (always, in every adapter) | `bundled/zcode.yaml`, `ide.rs:245`, `bundled/codex.yaml` |
| Skills source on disk | `<root>/versions/<v>/<PluginId>/skills` (zcode/claude/codex via `integrate_<ide>`) or `<root>/versions/<v>/skills/<BundleId>/` (opencode via `home.skills_root(version)`) | `ide.rs:362-366`, `install.rs:55` |

The IDE adapters read from a per-plugin `versions/<v>/<PluginId>/skills/`
path in the legacy code, but the opencode path uses the canonical
`versions/<v>/skills/` from the bundle world. There are two parallel
"where do skills live" codepaths in the same source file. L4 retargeted
the opencode path; the other three adapters are still on the legacy
join (this is a separate fixup, not part of DEBT-3).

## 5. Legitimate derivations vs. heuristics

Per the cycle's contract:

> Una identidad no puede derivarse de otra sólo porque hoy tengan
> nombres parecidos.
>
> Prohibido como contrato:
> plugin == component
> component == binary
> artifact stem == component
> skill bundle == plugin
>
> salvo que una relación explícita del manifiesto lo declare.

### 5.1 Legitimate derivations (declared in a manifest)

| Derivation | Declared by | Notes |
|---|---|---|
| `ComponentId == ArtifactKind::stem()` | `BundleComponent.name` MUST equal `kind.stem()` (bundle_manifest.rs:93) | This is a compile-time invariant of the bundle parser. |
| `BinaryName == PluginManifest.binaries[].name` | `plugin.yaml::binaries[]` field | The bundle world does not model binaries; the plugin world does. Today the bundle world subsumes `mcp-server`'s `cognicode-mcp` because `PluginManifest.binaries[].name == ArtifactKind::DaemonCli.stem()`. |
| `Skill bundle → requires PluginId` | `SkillManifest.requires: Vec<String>` | A skill says "I need plugin X". This is the only declared plugin↔skill cross-reference. |
| `Plugin → owns binaries` | `PluginManifest.binaries[]` field | One plugin can ship many binaries. |

### 5.2 Today's heuristics that need explicit replacement

| Heuristic | Where | Status |
|---|---|---|
| `BinaryName == "cognicode-mcp"` (literal) | `install.rs:67`, `ide.rs:730`, `bundled/{zcode,claude,codex}.yaml` | **WIDESPREAD LITERAL.** 5 hardcoded occurrences. Any future component with a different binary name (e.g. `ExplorerApi` → `explorer-api`) breaks without a separate co-change in those files. |
| `BundleComponent.name == "cognicode-mcp"` (literal) | `installer_transaction.rs` shape `bin/<comp>/<comp>` | The install script assumes the binary inside the bundle is named `bin/<comp.name>/<comp.name>`. This is implicit but not declared. |
| "First directory under `skills_root`" | `install.rs:56-59` ("L4 picks the first directory under `skills_root` if any exists, or skips integration with a warning") | The skill bundle manifest today does not declare *which* bundle to integrate; L4 falls back to "first". This is DEBT-2 territory but it surfaces here as a cross-identity heuristic. |
| `"mcp-server"` literal in install.rs:46-49 (pre-L4) | Replaced by L4; now uses `home.skills_root(version)`. | Reference for the WU5 invariant test. |
| `PluginId == "mcp-server"` arg in tests | `layout.rs:774-775, 1597, 1634, 1666, 1849`; `ide.rs:877,981,1042,1094`; `lifecycle.rs:161-349`; `lockfile.rs:60,80,81` | Tests use `"mcp-server"` as a string literal. This is a *test fixture*, not identity-derived logic. Acceptable in test scope; flag if any non-test code path also depends on the literal. |

### 5.3 Eliminations plan

| Heuristic | Plan |
|---|---|
| `BinaryName == "cognicode-mcp"` literal | Replace each literal with a derivation from `BundleManifest` (e.g. `manifest.components_by_kind(ArtifactKind::DaemonCli).first().map(|c| c.name.as_str()).unwrap_or("cognicode-mcp")` as fallback). Test pinned in WU5. |
| `bin/<comp>/<comp>` implicit shape | Add a test that verifies the extracted shape comes from the bundle's actual layout, not from a naming assumption. See WU5 T3. |
| "First directory under `skills_root`" | This is DEBT-2. Out of scope for DEBT-3; tracked in `cognicode-mcp` skill bundling follow-up. |
| `"mcp-server"` literal in tests | Acceptable. Pinned as test fixtures; the WU5 invariant test guards against the *production* code path re-introducing the literal. |

## 6. WU4 — compatibility decision

The user raised four candidate cases. Each gets a verdict + justification
here. The veridct is `proposed` in this ADR; ratifying the ADR closes
the decision.

### 6.1 `mcp-server` (PluginId)

Verdict: **(A) — valid PluginId containing `cognicode-mcp` (DaemonCli
component) + skill bundles (`cognicode`, `cognicode-mcp`)**.

Why: the PluginId appears in user-facing CLI commands, in the lockfile,
in the on-disk plugin directory, and as the canonical first-party plugin
identity. The bundle manifests are a parallel system (introduced for
the co-versioned Layer 0/1 model), not a replacement for the plugin
world. The plugin world is what the bundled `mcp-server.yaml` and the
`PluginManifest` struct declare. There is no need to retire the plugin
name.

Implicit relationship (declared): `mcp-server` (plugin) ships one binary
`cognicode-mcp` (per `bundled/mcp-server.yaml::binaries[].name`). Implicit
relationship (not declared, observed): the plugin's host bundle is the
`DaemonCli` artifact in `BundleManifest`. DEBT-3 makes this observed
relationship explicit (see §7).

### 6.2 `cognicode-core` (SkillBundleId, archived)

Verdict: **(B) — legacy to retire**, with a one-release deprecation
window during which `requires: [mcp-server]` and `cognicode-core` are
both valid, after which `cognicode-core` is rejected from any new
`SkillManifest`.

Why: the e84.1-skill-productization cycle archived
`cognicode-core-archived/` and `cognicode-mcp-driven-archived/`. The
live skill bundles are `cognicode/` and `cognicode-mcp/`. Keep the
archived path discoverable (so users can migrate) but no new code path
should accept `cognicode-core` as a live identity.

Compatibility shim (proposed): if a `SkillManifest.name ==
"cognicode-core"` or `"cognicode-mcp-driven"` is encountered, log a
"deprecated" warning at parse time and route the install to the
canonical replacement (`cognicode`, `cognicode-mcp`). The shim is
itself test-pinned via a `SkillManifestCompat` test (see WU5 T6).

### 6.3 `cognicode-mcp-driven` (SkillBundleId, archived)

Same as 6.2.

### 6.4 `cognicode` (SkillBundleId and ComponentId, both live)

Verdict: **(D) — intentionally shared name across two namespaces**.

`cognicode` is simultaneously the name of the runtime component
(`ArtifactKind::Cognicode`) and the name of a portable skill bundle.
These are unrelated concepts that happen to share a string. The
identities are disambiguated by *where they appear*: under
`versions/<v>/cognicode/bin/...` it is a component; under
`versions/<v>/skills/cognicode/...` it is a skill bundle. The names
share, but the resolved identities never collide at the filesystem
level.

No alias needed; the namespace IS the disambiguator. Tests in WU5 pin
that `component_root(version, "cognicode")` and
`skill_bundle(version, "cognicode")` return disjoint paths.

## 7. Direction of preference for a future model

This is the *direction*, not yet a design decision. Recorded here so
DEBT-2 has a starting point but does not couple to this cycle.

```text
PluginManifest                  (e32 plugin world)
    owns/requests:
        - BinaryName[]
        - capabilities[]         (future)
        ↓
SkillManifest                   (portable skill bundle world)
    requires:
        - PluginId[]
    ↓ skills installed under versions/<v>/skills/<SkillBundleId>/
    ↓ via per-IDE adapter → ~/.config/<ide>/skills/<BundleId>/
```

BundleManifest (co-versioned Layer 0/1 model, currently owned by
e85) is a parallel system that produces:

```text
versions/<v>/<ComponentId>/
    bin/<BinaryName>?
bin/<BinaryName>  → shims/<BinaryName>
versions/<v>/manifest.yaml
versions/<v>/skills/<SkillBundleId>/  (via plugin delivery)
```

The bundle world is the *producer*. The plugin world is the
*delivery mechanism*. The two are not redundant; they answer
different questions (bundle: "what does a release contain?"; plugin:
"how do I fetch + install it?").

A question that consumers should be able to ask without reconstructing
identities from names:

```text
component_root(version, ComponentId)
skills_root(version, ComponentId)            # per-component skill namespace
binary_path(version, ComponentId, BinaryName) # single executable
shim_path(version, ComponentId, BinaryName)
skill_bundle(version, SkillBundleId)
```

The first three exist today as layout-helper methods (`version_root`,
`component_root`, `skills_root`, `skill_bundle`). `binary_path` does
not. Adding it is bounded (one helper + tests) and is the natural next
slice of DEBT-3 work.

## 8. Cross-references

- `docs/adr/ADR-OWNERSHIP-MAP-install-vs-versions.md` (Phase A of
  `arch-canonical-layout`; predecessor ownership map for paths only).
- `docs/adr/ADR-CANONICAL-LAYOUT-versions.md` (the canonical layout).
- `openspec/specs/cognicode-cli/spec.md` (`plugin` argument shape).
- `openspec/specs/cognicode-plugin/spec.md` (`PluginManifest` v1).
- `openspec/specs/cognicode-ide-adapter/spec.md` (`IdeAdapter` v1).
- `openspec/specs/cognicode-lifecycle/spec.md` (`<plugin>` argument).
- `openspec/specs/portable-skill-bundle/spec.md` (`SkillBundleId`,
  `SkillManifest`, `requires: [PluginId]`).
- `crates/cognicode-cli/src/cmd/release_contract.rs` (Rust-side
  `ArtifactKind`, `COMPONENTS`, `kind.stem()`).
- `crates/cognicode-cli/src/cmd/bundle_manifest.rs` (`BundleManifest`
  v2 contract).
- `crates/cognicode-cli/src/cmd/manifest.rs` (`PluginManifest` v1).
- `crates/cognicode-cli/src/cmd/skill.rs` (`SkillManifest` v1).
- `crates/cognicode-cli/src/cmd/bundled/*.yaml` (the 6 bundled plugin
  manifests that exercise every identity combination).

## 9. Provenance

- Cycle initiative: `arch-identity-taxonomy` (DEBT-3).
- Investigation date: 2026-09-18.
- Reference commit: HEAD = `e4985597` (post-arch-canonical-layout L5
  archive).
- Real-PC UAT observable that originally motivated DEBT-3: Phase D
  reinstall `link_or_copy failed` (closed by L4 in commit `ab16e492`,
  but the underlying *naming* issue — why `mcp-server` was assumed to
  be a skill bundle's name — is what DEBT-3 addresses).
