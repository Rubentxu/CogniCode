# WU11 + WU12 + WU13 + WU20 — Skills.sh compatibility, source-of-truth, versioning, e85 relationship

These four WUs are documentation/specification work that lives
mostly in the skill `SKILL.md` and `manifest.yaml` files. This
document consolidates the design decisions and points to where
each is enforced.

## WU11 — Version-independent skill wording

**Goal:** Skill behaviour tracks capabilities, not release numbers
or internal milestone names.

**Implementation:**

- `SKILL.md` frontmatter declares `version: "1.0.0"` — the **skill
  version**, not the CogniCode binary version. This can be bumped
  independently of the binary.
- `manifest.yaml` declares a `compatibility: cognicode_runtime:
  ">=0.95"` block — minimum runtime version required. This is a
  **runtime-version constraint**, not a skill version.
- The skill body NEVER says "0.92.0", "0.95.0", "e31", or any
  internal milestone name (enforced by `validate_skills.py`).
- Tool-specific stability labels (`stable`, `experimental`) come
  from the runtime's `_meta.stability` field — not from the skill.

**Validation:** `scripts/validate_skills.py` rejects `0.92.x` and
several internal `just` recipe names.

## WU12 — skills.sh compatibility

**Goal:** Skills must be valid Agent Skills (skills.sh) with
optional CogniCode extensions.

**Implementation:**

- `SKILL.md` is the portable contract. It uses standard frontmatter
  fields only: `name`, `description`, `license`, `metadata`.
- The `description` field starts with "Trigger: When ..." so the
  Agent Skills indexer can match triggers correctly.
- No `manifest.yaml` is required by skills.sh. The skill works
  without it. `manifest.yaml` is the **CogniCode extension** for
  `cogh`-side lifecycle (requires, ide_compatibility,
  compatibility). It is OPTIONAL for skills.sh but required for
  CogniCode's local install path.
- Skill directories are flat: `skills/<name>/SKILL.md`. No nested
  layout, no hidden conventions.

**Validation:** `scripts/validate_skills.py` checks frontmatter
fields. `scripts/verify-skills.sh` (existing) checks YAML syntax
and required manifest fields.

## WU13 — Canonical source and skills.sh relationship

**Goal:** GitHub is the source of truth; skills.sh is discovery;
`cogh` is the official install path.

**Implementation:**

- The `SkillSource` model (WU14) defines a canonical source
  descriptor that does NOT depend on skills.sh.
- `cogh` MUST NOT depend on `npx`, Node, or skills.sh's internal
  API (recorded as a constraint carried into e86).
- skills.sh outage MUST NOT prevent `cogh skill install`
  (recorded as a constraint).
- Skills.sh index presence is a **discovery concern**, not a
  runtime authority. Direct GitHub installation is the technical
  acceptance criterion for the spike (WU16).

## WU20 — Relationship to e85 (Linux Release Factory)

**Goal:** Skills are NOT packaged into the binary tarball.

**Implementation:**

- Skills live in the **git source** at `skills/<name>/SKILL.md`.
- Skills are NOT bundled into `cognicode-cli`, `cognicode-mcp`, or
  any other binary.
- e85 (Linux Release Factory) ships only the binaries; the skill
  bundle is fetched from git by `cogh` (future) or by an agent's
  skills loader.
- An optional air-gapped skill bundle tarball MAY be a future
  capability (offline install) but is NOT required by e85.
- The new `cognicode-recommended` SET.yaml is also in the source
  tree, not the binary tarball.

**Consequence:** e85 can ship without waiting on a skill bundle.
The two pipelines are decoupled.

## Validation summary

The four WUs are validated by:

| Check | Where |
|-------|-------|
| No stale versions in skills | `validate_skills.py` (regex) |
| No `target/debug/` paths | `validate_skills.py` (regex) |
| No internal doc references in user skills | `validate_skills.py` (regex) |
| Frontmatter fields present | `validate_skills.py` |
| Tool refs in catalog | `validate_skills.py` |
| YAML manifests parse | `verify-skills.sh` |
| skills.sh-compatible frontmatter | manual + validator |

If all checks pass, the four WUs are considered satisfied.
