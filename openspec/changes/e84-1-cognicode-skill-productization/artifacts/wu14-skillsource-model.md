# WU14 — `SkillSource` identity model (metadata only, no install)

The brief defines the future source descriptor conceptually. This
cycle formalises it as a **metadata-only** model. No implementation
of `cogh skill install` is included.

## Model

```yaml
SkillSource:
  repository: <git URL>           # canonical source repo
  git_ref:    <tag | branch | commit SHA>
  path:       <path within repo>  # e.g. "skills/cognicode-mcp"
  content_digest: <sha256>        # of the SKILL.md body + manifest.yaml
```

### Fields

| Field | Purpose | Example |
|-------|---------|---------|
| `repository` | Identifies the canonical source. | `https://github.com/Rubentxu/CogniCode` |
| `git_ref` | Pin: latest tag, exact tag, branch, or commit SHA. | `v0.95.0`, `main`, `46ca3c48b176...` |
| `path` | Subdirectory within the repo holding the skill. | `skills/cognicode-mcp` |
| `content_digest` | Content-addressed integrity check. | `sha256:7c3f...` |

### Resolution modes (supported)

| Mode | git_ref | Notes |
|------|---------|-------|
| `latest` | resolves to most recent tag matching the skill | requires network; falls back to cached offline copy |
| `pinned-tag` | exact tag, e.g. `v0.95.0` | reproducible |
| `pinned-commit` | exact SHA | strictest reproducibility |
| `offline-cached` | (git_ref absent) | uses the locally-cached digest |

### Where this lives in the manifest

Today: implicit (the user installed a skill from somewhere). The
future format includes it explicitly:

```yaml
apiVersion: cognicode/v1
kind: SkillBundle
name: cognicode-mcp
version: "1.0.0"
source:
  repository: https://github.com/Rubentxu/CogniCode
  git_ref:    v0.95.0
  path:       skills/cognicode-mcp
  content_digest: sha256:7c3f...
```

## What this is NOT

- This is **not** a `cogh skill install` implementation. None of
  this is wired into `cogh` in this cycle.
- This is **not** a `skills.sh` private API. The descriptor is
  CogniCode-owned and does not require skills.sh.
- This is **not** a replacement for the runtime catalog. The
  runtime's `tools/list` is the source of truth for what tools a
  skill teaches, not what the skill's source location is.

## Constraints carried into e86 (future)

1. `cogh skill install` MUST work offline against a cached
   `SkillSource` if the network is unavailable.
2. `cogh` MUST NOT depend on `npx`, Node, or skills.sh internals.
3. `content_digest` MUST be re-verified after every fetch; a
   mismatch is a hard error.
4. Skills installed from sources other than
   `Rubentxu/CogniCode` (the canonical repository) MUST be
   visibly marked as "third-party" by `cogh skill list`.

## Why this matters

Without this model, every skill source is "wherever you got it
from". With it, every installed skill has a verifiable identity
that can be:

- Re-fetched from the canonical source on upgrade.
- Verified against the content digest to detect tampering.
- Listed in `cogh skill list` with provenance.
- Diffed against an installed copy to show what changed.

## Status

Documented as a model. Implementation deferred to e86
(`cogh skill install`).
