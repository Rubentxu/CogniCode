# WU15 — `cognicode-recommended` (set/profile)

This is **not** another `SKILL.md`. It is a curated **set/profile**
that lists which skills should be installed together for a typical
user.

## File

`skills/cognicode-recommended/SET.yaml` — a YAML manifest describing
the set.

```yaml
apiVersion: cognicode/v1
kind: SkillSet
name: cognicode-recommended
description: |
  Curated bundle of user-facing CogniCode skills. The default install
  for any user who wants "CogniCode, just working".
version: "1.0.0"
maturity: stable

# Skills in this set, in load order. Each entry is a SkillSource
# pointer; resolution rules are in WU14.
includes:
  - source: skills/cognicode
  - source: skills/cognicode-mcp

# Skills explicitly NOT included (developer-only, not for users).
excludes:
  - skills/cognicode-developer
```

## Why this is a SET, not a SKILL

- A `SkillSet` is **a list of skills to install**. It is consumed
  by the future `cogh skill install cognicode-recommended` (e86).
- A `Skill` (`SKILL.md`) is **a single coherent teaching unit** that
  an agent loads. It is consumed by the agent runtime.

A `SkillSet` is a meta-artifact; it has no body for an agent to
read. It exists so that "give me the recommended skills" is a
single, declarative concept.

## Recommended content (initial)

| Skill | Why included |
|-------|--------------|
| `cognicode` | Entry point — install verification, surface choice, first discovery. |
| `cognicode-mcp` | The MCP workflow guide — covers DISCOVER/NAVIGATE/GRAPH/IMPACT/QUALITY/SAFE CHANGE/CODE IO/INFRASTRUCTURE. |

## Explicitly excluded

| Skill | Why excluded |
|-------|--------------|
| `cognicode-developer` | Developer-only. Loading it in a user-agent is product-fiction — it teaches `cd` into the CogniCode repo and `just` recipes the user does not have. |
| `cognicode-architecture` / `cognicode-impact-analysis` / `cognicode-safe-refactor` (future, deferred) | Narrower skills. Not yet needed; may be added in a future cycle if workflows get distinct enough. |

## Loading order

The `includes` list is ordered. Agents should load skills in this
order:

```text
cognicode (entry point, install verification)
   ↓
cognicode-mcp (workflow guide)
```

Loading `cognicode-mcp` without first loading `cognicode` is
allowed but suboptimal — the entry-point skill handles the
CLI/MCP routing decision that the workflow skill expects the
agent to have made.

## Why not just publish each skill individually?

Skills.sh and the Agent Skills ecosystem support publishing
individual skills. We **also** publish individuals:

- `skills/cognicode/SKILL.md` — installable individually.
- `skills/cognicode-mcp/SKILL.md` — installable individually.

The set is a convenience, not a constraint. A user who wants
**only** `cognicode-mcp` (because they have already done
orientation in a previous session) can install just that.

## Status

Documented as a profile. Implementation of `cogh skill install
cognicode-recommended` deferred to e86.
