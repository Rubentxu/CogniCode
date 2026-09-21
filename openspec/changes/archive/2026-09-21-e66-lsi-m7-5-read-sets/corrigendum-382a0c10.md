# e66 / e67 — corrigendum to commit 382a0c10

The commit message of `382a0c10` (`docs(e66,e67): land final SDDK cycle artifacts
for archive`) contains two claims that are **inaccurate**:

1. *"the archive phase can sync the delta specs into openspec/specs/"* — no delta
   specs were synced. The `openspec/specs/` directory does not currently contain a
   read-sets spec or a production-grounding spec derived from the e66/e67 proposals.
   Whether delta specs SHOULD be added is a separate decision (per AGENTS.md,
   *"Requirement ids stay in the umbrella change; evolutivos carry no duplicate spec
   delta"*) — for e66 (foundation, no spec delta) and e67 (no spec delta, the cycle
   refines M6 at the producer + gate boundaries) the answer is "no delta spec needed",
   but the commit message did not state that.

2. *"the ledger can mark both cycles as fully archived"* — neither cycle is in a
   state that a normal SDDK reader would call "fully archived":
   - e66 is `CLOSED` phase `archive` in the ledger, but that closure was reached via
     a SQLite shortcut transition to bypass the `ENGINE_MISSING_GATE_RECEIPT` bug
     (DEBT-SDDK-002 § 1). It is **administrative closure, not normal orchestrator
     closure**.
   - e67 has no ledger row at all (DEBT-SDDK-003: cycle was never instantiated).

## Correct status (this corrigendum)

| Cycle | Code | Artifacts | Verification | Ledger state | Archive semantic |
|-------|------|-----------|--------------|--------------|------------------|
| e66 | committed | committed (this commit) | GREEN | CLOSED via D3 defer | administrative |
| e67 | committed | committed (this commit) | GREEN | no row (cycle never instantiated) | on-disk record |

For the honest closure semantics, see:
- `docs/debts/DEBT-SDDK-002.md` (affected cycles + closure semantics fields)
- `.agent/TESTING-STATE.md` (closure matrix with `D3-DEFER` marker)
- `openspec/changes/e66-lsi-m7-5-read-sets/archive-manifest.md`
- `openspec/changes/e67-lsi-production-grounding/archive-manifest.md`

## Why this is not a force-amend

The commit `382a0c10` is already on `origin/main` (pushed during the 71-commit
release integration on 2026-09-17). A `git commit --amend` would require a
force-push, which:
1. would rewrite a public SHA,
2. would invalidate the v0.95.0-m6-m7-closure tag (the tag points at
   `382a0c10c3a0dc65b4abded070fd75a5f4f410a3`),
3. would be more invasive than a corrigendum commit.

A corrigendum commit is the honest alternative: acknowledge the inaccurate
statement in writing, on disk, in a way that is auditable and reversible.

## Future commit hygiene

Going forward, commit messages must:
- NOT anticipate future actions as if they had happened ("so the X can sync Y"),
- use past tense for completed actions ("landed", "archived", "closed via D3 defer"),
- use conditional tense for hypothetical follow-ups ("a future cycle may sync…"
  vs "the archive phase can sync…").
