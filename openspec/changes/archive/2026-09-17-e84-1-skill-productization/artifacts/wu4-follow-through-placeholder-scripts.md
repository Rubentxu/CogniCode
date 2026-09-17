# WU4 follow-through — Placeholder references/scripts fate

## What was found

`openspec/changes/archive/2026-09-17-e84-1-skill-productization/
artifacts/old-skills/cognicode-mcp-driven-archived/references/`
contains two placeholder scripts:

- `bootstrap.sh` — 2-line placeholder (`echo "cognicode-mcp-driven
  reference bootstrap"`).
- `invoke.sh` — 4-line placeholder with `set -euo pipefail` and a
  `echo "(cognicode helper — placeholder)"` exit-0.

Both are clearly stale. Neither is referenced by the archived
`SKILL.md` (checked at audit time). Neither is referenced by the
new live `cognicode-mcp` skill.

## Decision

**Leave the placeholders in the archive as-is.** Reasons:

1. The archive is the historical record of what e84.1 found and
   replaced. Removing them is retroactive editing of the evidence.
2. The new `validate_skills.py` already excludes archived skills
   (only walks `skills/`), so the placeholders have zero live
   impact.
3. A future audit that compares archived vs new skills benefits
   from seeing the diff, including the placeholder removal.
4. Cost of leaving: ~10 lines of dead shell in a tree that is
   not loaded by anything. Cost of removing: one extra commit
   per future reader who tries to follow the audit trail.

## Live skills reference scripts?

None of the 3 new skills (`cognicode`, `cognicode-mcp`,
`cognicode-developer`) or the SET (`cognicode-recommended`) ship
with `references/*.sh`. The new skills are **declarative
Markdown + YAML**, with no executable helpers — agents consume
the skill text and call the live CLI/MCP runtime directly.

This is intentional and matches the e84.1 brief: "no install
impl", "no runtime deps". Skills.sh compatible frontmatter only;
the user brings their own runtime.

## What this means for future skills

- A future "operator" or "release-engineering" skill might
  include real reference scripts. Those would live under
  `references/` and be invoked via `bash $SKILL_DIR/references/...`
  from the agent runtime (skills.sh convention).
- Until then, the absence of `references/` is **by design** and
  is not a gap.
