---
title: "ADR — Canonical installation layout: `versions/<v>/<component>/`"
slug: "ADR-CANONICAL-LAYOUT"
status: proposed
date: 2026-09-18
deciders: Maintainer
related:
  - "[[ADR-034-cognicode-distribution-package]]"
  - "[[ADR-035-asdf-vm-version-management-pattern]]"
  - "[[ADR-036-ide-abstraction-portable-skills-per-ide-adapters]]"
supersedes: "The `install/<v>/...` layout used by `InstallerTransaction` (e74-era implementation choice, never ratified by an ADR)"
context:
  - "E86 UAT Phase D reinstall fails `link_or_copy` because the install transaction writes to `install/<v>/...` while the IDE adapters and every promoted OpenSpec spec promise `versions/<v>/...`"
  - "Six bounded cycles (E86.2.3 through E86.7) closed surrounding bugs but did not address the layout contradiction"
  - "ADRs 034/035 and three OpenSpec specs all say `versions/<v>/...`; only the runtime code says otherwise"
---

# ADR — Canonical installation layout: `versions/<v>/<component>/`

## Context

CogniCode installs a runtime composed of:

1. **Runtime components** (binaries): `cognicode`, `cognicode-mcp`,
   `explorer-api` (per `ArtifactKind::stem()` in
   `crates/cognicode-cli/src/cmd/release_contract.rs:146`).
2. **Portable skill bundles**: hand-built `.tar.gz` archives containing
   `SKILL.md`, `assets/`, `references/`, `manifest.yaml`. Currently
   published as `cognicode-core-<v>.tar.gz`,
   `cognicode-mcp-driven-<v>.tar.gz`, etc.

The runtime code has two competing layouts:

| Layout | Producer | Promoted spec |
|---|---|---|
| `<root>/install/<v>/<comp>/bin/<comp>` | `InstallerTransaction` (writes) | none |
| `<root>/versions/<v>/<plugin>/bin/<binary>` | none (everyone reads) | ADR-034, ADR-035, `cognicode-cli/spec.md`, `cognicode-lifecycle/spec.md`, `portable-skill-bundle/spec.md` |

ADR-034 explicitly states the canonical layout:

```text
~/.cognicode/versions/0.92.0/
├── bin/
│   ├── cognicode-mcp
│   ├── cognicode-explorer-api
│   └── cognicode-cli
├── skills/
│   ├── cognicode-core/
│   │   ├── SKILL.md
│   │   └── scripts/
│   ├── cognicode-mcp-driven/
│   └── cognicode-sandbox/
```

ADR-035 ratifies the asdf-inspired `versions/<v>/` pattern as
intentional simplification (CogniCode is single-tool, multi-version,
not multi-tool).

Three OpenSpec specs are promoted and bind the user-visible contract:

- `cognicode-cli/spec.md:42` — `~/.cognicode/versions/<v>/<plugin>/bin/...`
- `cognicode-lifecycle/spec.md:40,63,77` — `~/.cognicode/versions/<v>/`
- `portable-skill-bundle/spec.md:36,138` — `~/.cognicode/versions/<v>/skills/`

The `install/` layout in current code is an undocumented e74-era
implementation choice. e74 closure (`openspec/changes/e74-lsi-portable-runtime-distribution/closure-authorization.md`)
records it as a "low severity" debt to be addressed when e76 introduces
isolated installs. It was never promoted to an ADR.

## Decision

**The canonical installation layout is `~/.cognicode/versions/<v>/<component>/`.**

```text
CognicodeHome
└── versions/
    └── <version>/
        ├── manifest.yaml                    # BundleManifest snapshot
        ├── bin/
        │   ├── cognicode                    # symlink or copy into component root
        │   ├── cognicode-mcp
        │   └── explorer-api
        ├── skills/
        │   ├── cognicode-core/
        │   │   ├── SKILL.md
        │   │   ├── manifest.yaml
        │   │   └── ...
        │   └── cognicode-mcp-driven/
        └── <component>/                     # one child per bundle component
            └── ...
```

Concrete path invariants:

1. **One installed release** → one version root: `<root>/versions/<v>/`.
2. **One bundle component** → one child under that version root:
   `<root>/versions/<v>/<component>/`. The component name is the
   `BundleComponent::name` field (= `ArtifactKind::stem()`).
3. **The manifest** lives at `<root>/versions/<v>/manifest.yaml`.
4. **The skill bundles** live at `<root>/versions/<v>/skills/<bundle>/`.
   They are a separate axis from runtime components: a skill bundle is
   a self-contained artefact extracted flat from its tarball.
5. **The shims** live at `<root>/shims/` and reference the version root
   via relative or absolute paths (existing E32-A contract).
6. **The tracker** lives at `<root>/tracker/version` (existing contract).

`install/` is **not** part of the canonical layout. It exists only as
a private staging area for atomic install transactions
(see "Staging vs canonical" below). After a successful commit, no
`install/<v>/` tree is on disk; only `versions/<v>/` remains.

### Staging vs canonical

Atomic installs require a temporary working area distinct from the
canonical tree. The transaction MAY use a private staging directory,
but the staging directory MUST NOT be part of the user-visible
contract and MUST be cleaned up after commit.

The staging area candidates are:

- `<root>/.staging/install-<uuid>/`
- `<root>/.tmp/install-<uuid>/`
- `std::env::temp_dir()/cogh-install-<uuid>/` (preferred; leaves the
  home free of transient state)

The choice is implementation-detail of the transaction, not part of
this ADR. The pinned invariant is: after `InstallerTransaction` reaches
`Committed`, the user-visible state on disk lives under `versions/<v>/`.

### Producer/consumer invariants

This ADR pins the following ownership:

| Role | Surface | Path |
|---|---|---|
| Producer (write) | `InstallerTransaction::commit` | `<root>/versions/<v>/...` |
| Consumer (read) | `cmd_uninstall` | `<root>/versions/<v>/` |
| Consumer (read) | `cmd_ide_install` (all IDEs) | `<root>/versions/<v>/<plugin>/skills` |
| Consumer (read) | `cmd_ide_uninstall` (all IDEs) | external (IDE-side) |
| Consumer (read) | `cmd_doctor`, `cmd_current`, `cmd_latest`, `cmd_where` | `<root>/versions/<v>/...` |
| Consumer (read) | `cmd_rollback` | journal envelope (which itself references `<root>/versions/<v>/...`) |
| Consumer (read) | `cmd_list` | `<root>/versions/` (one entry per installed release) |

No module may derive one of the canonical paths by `root.join(...).join(...)`
chains. Each path is exposed via a typed helper on `CognicodeHome`:

```rust
impl CognicodeHome {
    /// Canonical root for a given installed version.
    pub fn version_root(&self, version: &str) -> PathBuf;

    /// Canonical root for a single bundle component within a version.
    pub fn component_root(&self, version: &str, component: &str) -> PathBuf;

    /// Canonical manifest path for a given version.
    pub fn version_manifest(&self, version: &str) -> PathBuf;

    /// Canonical skills root for a given version.
    pub fn skills_root(&self, version: &str) -> PathBuf;

    /// Canonical skills root for a single portable skill bundle.
    pub fn skill_bundle(&self, version: &str, bundle: &str) -> PathBuf;
}
```

(These five helpers are exactly the L1 deliverable.)

### Tarball-extract contract

`InstallerTransaction::Extracting` performs a flat extract: it
unpacks each `.tar.gz` archive into the component root without adding
or stripping any prefix. The bundle contract is:

```text
archive <component>-<version>-<platform>.tar.gz
└── <component-specific files>           # NO top-level <component>/ prefix
```

After extraction:

```text
versions/<v>/<component>/
└── <component-specific files>
```

If a runtime component archive contains a `bin/` directory, the
transaction's `InstallingShims` stage creates one shim per executable
found under `bin/`. If the archive does NOT contain a `bin/`
directory (e.g. it is a portable skill bundle), the shim stage is a
no-op for that component; the bundle itself is the deliverable.

This makes the install transaction work correctly for both kinds of
artefacts without hardcoding "every component has a `bin/`".

### Legacy migration

Users who installed an e74-era `install/<v>/` tree have on-disk state
that does not match this ADR. Two options:

1. **Auto-migrate on next `cogh install`**: detect `install/<v>/` and
   move it to `versions/<v>/` before extracting.
2. **Bail loudly**: `cogh` refuses to install if `install/<v>/` already
   exists and asks the user to run `cogh repair` (a new command).

Option 1 is friendlier; option 2 is safer for forensics. Both are
implementation-detail of L1-L5. Pinned invariant: dual-read is not
shipped. Either the installer migrates and writes `versions/`, or it
refuses and tells the user to migrate manually.

### Out of scope

These are pinned separately and not addressed by this ADR:

1. **`write_json_atomic` re-serialisation drift.** When the value being
   written is semantically identical to the existing on-disk content,
   `write_json_atomic` still rewrites the file with normalised
   whitespace, changing the sha256. UAT disposable verification
   should not flag this as pollution. Tracked as separate debt.

2. **Journal + uninstall retention policy.** `cmd_uninstall` removes
   the install tree; the journal entry remains (by design: rollback
   can recreate the tree from the journal). Whether uninstall
   should also prune the journal is a separate policy decision.

3. **Plugin-vs-component naming.** The promoted OpenSpec specs use
   `<plugin>`; the bundle manifest uses `<component>`; the dev fixture
   uses `mcp-server` (spec) vs `cognicode-mcp` (bundle). The names
   are interchangeable for the purposes of this ADR (the child node
   under `<v>/` is whatever the bundle manifest says); reconciling
   them is a separate documentation cycle.

## Consequences

### Positive

- Every promoted spec and ADR becomes load-bearing again. No
  implementation silently contradicts them.
- The IDE adapters (which were already correct) start working
  end-to-end because the install transaction writes where they read.
- `cmd_uninstall` (E86.7) removes the correct tree.
- The UAT Phase D reinstall `link_or_copy` becomes GREEN because the
  IDE integration derives `skill_path` from the canonical component
  root and the `mcp-server` vs `cognicode-mcp` confusion is removed.
- `cmd_doctor`, `cmd_where`, `cmd_current` all observe a single tree.

### Negative

- L1-L5 migration requires touching the install transaction, the IDE
  adapter, `cmd_uninstall`, `cmd_doctor`, and ~6 test fixtures.
  Estimated bounded-cycle count: 5 cycles (L1-L5), each ~5-line + 2-3
  tests.
- Users with on-disk `install/<v>/` from e74-era installs need a
  migration step (auto or manual, see above).
- The free fn `layout::install_manifest_path` and the `CognicodeHome`
  method `install_manifest_path` (E86.4) need to be retired or renamed.

### Risk

- The risk is concentrated in L2 (changing the producer). If L2 fails
  to land cleanly, the install transaction writes a partial tree to
  `versions/<v>/` and the rollback journal is the only safety net.
  Mitigation: L2 ships with characterization tests that pin the
  on-disk shape before and after.
- The portable skill bundle question (whether they enter the home via
  the bundle manifest or via the legacy `cogh plugin add` path) is
  unresolved at this ADR's scope. The L4 cycle must either resolve it
  or file a follow-up.

## Alternatives considered

### B1: Promote `install/` to canonical and update the specs.

Rejected. ADR-034 and ADR-035 are accepted; rebasing three promoted
specs against the implementation is a larger surface than fixing the
implementation against them. The user-visible semantic
(`versions/<v>/<plugin>/`) is the right one for a CogniCode-style
single-tool, multi-version, plugin-aware distribution.

### B2: Coexist (`install/` for transaction, `versions/` for consumer).

Rejected by the user's explicit "no dual-read forever" directive.

### B3: One unified `install/<v>/<tool>/` (asdf-style, multi-tool).

Rejected. CogniCode is single-tool. ADR-034's `versions/<v>/`
simplification is intentional.

### B4: Use `versions/<v>/<component>/` for runtime, keep
`skills/<bundle>/` separately under `<root>/skills/<bundle>/` (not
versioned).

Rejected. `portable-skill-bundle/spec.md` already promotes
`versions/<v>/skills/<bundle>/`. Per-version skill versioning is
the right semantic for lifecycle (an update can change a skill
without breaking the runtime).

## Compliance

This ADR is binding on:

- `crates/cognicode-cli/src/cmd/installer_transaction.rs`
- `crates/cognicode-cli/src/cmd/install.rs`
- `crates/cognicode-cli/src/cmd/layout.rs`
- `crates/cognicode-cli/src/cmd/ide.rs`
- `crates/cognicode-cli/src/cmd/registry.rs`
- `crates/cognicode-cli/src/cmd/rollback_journal.rs`
- `crates/cognicode-cli/src/cmd/lifecycle_journal.rs`

It does NOT change:

- The bundle manifest format (`bundle_manifest.rs`).
- The release contract (`release_contract.rs`).
- The portable skill bundle format (`SKILL.md`/`manifest.yaml`).
- The shim format (E32-A).

## Implementation roadmap

This ADR is the entry gate for the L1-L5 bounded migration:

| Cycle | Subject | Bounded |
|---|---|---|
| L1 | Typed layout ownership (5 helpers + characterization tests) | no behaviour change |
| L2 | InstallerTransaction producer switches to `versions/<v>/<comp>/` | behaviour change in producer |
| L3 | Lifecycle consumers (uninstall, rollback, doctor, current, where) | consumer retargeting |
| L4 | IDE consumers (cmd_ide_install, install.rs skill_path derivation, UAT GREEN) | consumer retargeting |
| L5 | Legacy layout elimination (free fns, E86.4 method, dual-read, e74 migration) | remove dead surface |

Each cycle is ~5-line + 2-3 RED tests + apply + archive + UAT re-run.
Auto-continues unless a phase surfaces a contradiction.
