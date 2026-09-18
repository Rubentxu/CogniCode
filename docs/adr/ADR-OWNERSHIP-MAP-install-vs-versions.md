# Ownership map — Canonical installation layout (install/ vs versions/)

> Companion to the architectural decision on `~/.cognicode/install/<v>/...`
> vs `~/.cognicode/versions/<v>/...`.
> Produced during Phase A of the architectural cycle `arch-canonical-layout`.
> All path references are RELATIVE to `<root>` (= `CognicodeHome::root`).

## 1. Authoritative contract sources

| Source | Stated canonical layout | Status |
|---|---|---|
| `docs/adr/ADR-034-cognicode-distribution-package.md` | `~/.cognicode/versions/<v>/bin/`, `versions/<v>/skills/` | accepted 2026-08-10 |
| `docs/adr/ADR-035-asdf-vm-version-management-pattern.md` | asdf's `installs/<tool>/<v>/` pattern; ADR-034 adapts to `versions/<v>/` for single-tool | accepted 2026-08-10 |
| `openspec/specs/cognicode-cli/spec.md` | `~/.cognicode/versions/<v>/<plugin>/bin/...` | promoted |
| `openspec/specs/cognicode-lifecycle/spec.md` | `~/.cognicode/versions/0.92.0/` | promoted |
| `openspec/specs/portable-skill-bundle/spec.md` | `~/.cognicode/versions/0.92.0/skills/cognicode-core/` | promoted |

**Contract**: every promoted contract says **`versions/<v>/...`** with a `<plugin>`
child node.

## 2. Producer side (who writes)

| Producer | Path written | Notes |
|---|---|---|
| `installer_transaction.rs:388-391` | `install/<v>/` (parent dir) | Transactional Extracting stage creates `<root>/install/<v>` as install dir |
| `installer_transaction.rs:395-398` | `install/<v>/<comp.name>/` | Each component extracted via `registry::extract_targz(&src, &install_dir.join(comp.name))` |
| `installer_transaction.rs:413-417` | `install/<v>/<comp.name>/bin/<comp.name>` | Shims point here; assumes component tarball contains a `bin/` dir |
| `installer_transaction.rs:616,635-637` | `install/<v>/manifest.yaml` | Manifest write path: `layout::install_manifest_path(&manifest.version)` |

The single producer (InstallerTransaction) writes to **`install/<v>/...`**.

## 3. Consumer side (who reads / deletes / links)

| Consumer | Path | Operation | Status |
|---|---|---|---|
| `layout.rs:31-38` free fns `install_root/install_dir` | `install/<v>` | helper | likely dead — replaced by `CognicodeHome::install_manifest_path` (E86.4) |
| `layout.rs:60-63` free fn `install_manifest_path` | `install/<v>/manifest.yaml` | helper | used by `installer_transaction::commit` |
| `layout.rs:97-102` `home.versions/version` | `versions/<v>` | helper | documented canonical; used by layout.rs tests |
| `layout.rs:103-108` `home.plugins/plugin` | `plugins/<name>` | helper | legacy `cmd_install` stub path |
| `layout.rs:143-148` `home.install_manifest_path` (method) | `install/<v>/manifest.yaml` | helper | created E86.4 to match free fn; used by `cmd_uninstall` |
| `installer_transaction.rs:386-431` `InstallStage::Extracting + InstallingShims` | `install/<v>/<comp>/...` | write + read | producer side (above) |
| `install.rs:46-49` `skill_path` derivation | `install/<v>/mcp-server/skills` | read | **broken**: bundle has no `mcp-server` component |
| `ide.rs:737` `cmd_ide_install` opencode branch | `versions/<v>/<plugin>/skills` | read | correct dir, but install transaction writes to `install/<v>/<comp>/`, not `versions/<v>/<plugin>/` |
| `ide.rs:355-369` `integrate_zcode` | `<root>/versions/<v>/<plugin>/skills` | read | same contradiction as opencode |
| `ide.rs:478,594` claude/codex `integrate_*` | `<root>/versions/<v>/...` | read | same |
| `ide.rs:230,257` `opencode_skills_dir` | external (`~/.config/opencode/skills`) | write | IDE-side target, not a layout question |
| `layout.rs:230-275` `cmd_uninstall` (just-closed E86.7) | `install/<v>/` (via `install_manifest_path().parent()`) | delete | matches what `installer_transaction` writes |
| `layout.rs:280-300` `cmd_list` | `plugins/<name>/` (legacy) | read | legacy plugin listing |
| `layout.rs:304-313` `cmd_current` | tracker | read | unrelated to layout |
| `layout.rs:316-345` `cmd_latest` | registry | read | unrelated |
| `layout.rs:394-440` `cmd_rollback` | journal | read | records paths via `SideEffect` (mixed) |
| `layout.rs:450-468` `cmd_doctor` | various | read | pin: see below |
| `layout.rs:460-468` `cmd_where` | various | read | see below |
| `ide.rs:253-280` `uninstall_opencode` | external (IDE-side) | delete | not a layout question |

## 4. Test fixtures

| Test | Path assumed | Notes |
|---|---|---|
| `ide.rs:877` `integrate_zcode_creates_mcp_section` fixture | `tmp/.cognicode/versions/0.92.0/mcp-server/skills` | uses `versions/`, plugin name `mcp-server` |
| `layout.rs:753,758,760` layout tests | `versions/`, `plugins/` | pinned to spec contract |
| `layout.rs:928-940` E86.3 uninstall test | `home_dir/install/<v>` (via `install_manifest_path().parent()`) | matches the install-side reality |
| `installer_transaction.rs:778` `commit_writes_manifest_file` test | `cognicode_home()/install/<version>/manifest.yaml` | matches the install-side reality |
| `installer_transaction.rs:833` `commit_writes_journal_next_to_install` test | journal next to install | same |
| `installer_transaction.rs:1066` etc. local-release tests | via TempCognicodeHome, install/<v>/ | same |

## 5. The contract-vs-reality mismatch

The promoted OpenSpec specs and ADR-034/035 promise:

```
<root>/versions/<v>/<plugin>/bin/<binary>
<root>/versions/<v>/skills/<skill-bundle>/
<root>/versions/<v>/manifest.yaml
```

The installer transaction actually writes:

```
<root>/install/<v>/<component>/bin/<component>
<root>/install/<v>/manifest.yaml
```

The IDE adapter `cmd_ide_install` reads:

```
<root>/versions/<v>/<plugin>/skills
```

The IDE integration in `install.rs:46-49` reads (BROKEN):

```
<root>/install/<v>/mcp-server/skills    ← no such component exists in any bundle
```

Net effect:

- The UAT Phase B install lands files under `install/<v>/`, never under
  `versions/<v>/`.
- The UAT Phase D reinstall fails `link_or_copy` because the opencode
  skills source path is hardcoded against a `mcp-server` component that
  the bundle does not declare.
- `cmd_ide_install --plugin mcp-server` would correctly look at
  `versions/<v>/mcp-server/skills`, but that directory is never created.
  Calling it standalone fails too (path does not exist).
- `cmd_uninstall` (E86.7) does the right thing for what was actually
  installed (it removes `install/<v>/`), not for what the spec promises.

## 6. Three plausible resolutions

### Option A: Spec wins. Move installer to write `versions/<v>/<comp>/`.

- **Pros**: aligned with every promoted spec and ADR. Removes the contradiction.
- **Cons**: existing installs at `install/<v>/` become legacy. Needs a one-shot
  migration warning for users with existing on-disk state.
- **Producer change**: 1 line in `installer_transaction.rs:388`
  (`layout::install_dir` → `home.version`). Plus matching shim source derivation.
- **Consumer change**: `install.rs:46-49` must derive `skill_path` from the
  correct component (look up by `kind == DaemonCli` or by `name`); not `mcp-server`.
- **Helper**: `CognicodeHome::install_manifest_path` retargets to
  `versions/<v>/manifest.yaml`. E86.4 test would need to update.

### Option B: Code wins. Update specs to `install/<v>/<comp>/`.

- **Pros**: no production change. Smaller cycle.
- **Cons**: breaks ADR-034/035 which are accepted; rebases
  cognicode-cli/cognicode-lifecycle/portable-skill-bundle specs against
  the implementation. Semantic difference: `install/<v>/<comp>/bin/<comp>`
  is "bundle-component-style" (one component = one bin tree), whereas
  `versions/<v>/<plugin>/bin/<binary>` is "plugin-style" (one plugin
  can have many bins).

### Option C: Coexist. Dual-write or dual-read.

- **Pros**: no immediate break.
- **Cons**: violates the user's explicit "no dual-read forever" directive.
  Two sources of truth.

## 7. Tarball shape evidence (Phase B preview)

The artefacts in `dist/` are:

```
cognicode-core-0.94.4.tar.gz     →  ./SKILL.md ./manifest.yaml
cognicode-mcp-driven-0.94.4.tar.gz →  ./SKILL.md ./assets/... ./references/... ./manifest.yaml
```

These are **portable skill bundles**, NOT binaries. They contain
`SKILL.md` at the top, plus `assets/` and `references/`. No `bin/`.

So the install transaction's assumption
`<root>/install/<v>/<comp>/bin/<comp>` (line 416) is structurally
incompatible with the real artefacts. Even if we move to `versions/`,
the install transaction would still look for `bin/<comp>/<comp>` inside
each extracted bundle, find nothing, and skip the shim (silent — see
`if bin_path.exists()` at line 417).

This means the install transaction's contract about WHERE to extract
is one issue, but its contract about WHAT to do with the extracted
content is another. The full resolution must address both.

The bundle manifest `dev-bundle.yaml` declares two components:
`cognicode` (kind: cognicode) and `cognicode-mcp` (kind: daemon-cli).
Neither is a skill bundle. So the skill bundles in `dist/` are
hand-built separately, not first-class bundle components.

**Open question for the ADR**: how do portable skill bundles enter the
home? Are they part of the bundle manifest (new `ArtifactKind::Skill`),
or are they installed separately via `cogh plugin add skills-cognicode`
(the legacy plugin path)?

## 8. Recommended decision (subject to ADR ratification)

**Adopt Option A**: `versions/<v>/<component>/` becomes the canonical
layout for runtime components. This aligns with every accepted ADR and
every promoted spec. The producer change is small; the consumer changes
are also small (the IDE adapters were already correct, they just
needed a writer that landed at the path they expect).

For portable skill bundles, the canonical placement under Option A is
`versions/<v>/skills/<skill-bundle>/`, exactly as `portable-skill-bundle/spec.md`
already promises.

The install transaction's `bin/<comp>/<comp>` shape is an E72-era
assumption that pre-dates the skill-bundle split. With real artefacts
in hand, that assumption is no longer load-bearing. The transaction
should:

1. Extract each component bundle to `versions/<v>/<comp>/`.
2. Inspect the extracted tree to find what kind of artefact it is
   (skill bundle with `SKILL.md` vs runtime binary with `bin/`).
3. Build shims only when a binary is actually present.
4. Surface a clear warning when a runtime component declares a binary
   but the archive doesn't have one (publish-time contract violation).

This is the L1-L5 bounded migration plan the user described.

## 9. Out of scope (carried separately)

- `write_json_atomic` re-serialisation drift (`13bb7c2e → b93fcd3c`).
  This is a separate UAT-level concern about touch-not-substance.
- Journal interaction with uninstall (`rollback == undo via journal`,
  `uninstall == remove installed state`). The ADR only needs to ensure
  both consumers know the canonical install root; the retention policy
  is a separate decision.
- Bundle component naming (`mcp-server` in spec vs `cognicode-mcp` in
  bundle manifest). Either is fine for the layout decision (the child
  name follows from whichever authoritative source wins). Logged for a
  later bounded cycle.

## 10. Provenance

- Cycle initiative: `arch-canonical-layout`.
- Investigation date: 2026-09-18.
- Reference commit: HEAD = `3d201325` (after E86.7 closure).
- Real-PC UAT observable: Phase D reinstall `link_or_copy failed`,
  surfaced repeatedly across e86 cycles.
