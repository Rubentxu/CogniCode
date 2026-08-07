# Session Resume — CogniCode v1.0.0 UAT

**Last session ended**: 2026-08-07
**Goal when resumed**: Execute UAT plan with human-in-the-loop, complete the 29 test cases, decide if v1.0.0 tag is ready.

---

## Where we left off

All technical work is **done**. Ready for human validation:

- **E2E suite**: 184 passed, 20 skipped, 0 failed
- **Visual deliverables**: 63 screencasts + gallery + scorecard shipped
- **UX polish**: 4 commits (P0 + P1 + P2/P3 + Round-2), score 23 → 29 → ~33
- **UAT plan**: drafted, validated, dashboards generated

The pending work is **human judgment**: run UAT, give verdicts, decide if v1.0.0 is ready.

---

## Resume commands (in order)

### 1. Confirm state

```bash
cd /var/home/rubentxu/Proyectos/rust/CogniCode
git log --oneline -8                    # verify last commit c5006fb2
git status --short                     # see any uncommitted work
sddk uat validate --file uat-plan.yaml # should be OK
```

### 2. Run the UAT (human-in-the-loop)

```bash
sddk uat open --plan uat-plan.yaml
```

This opens `uat-guided.html` in your browser. One scenario per screen, copy-paste evidence, verdict buttons.

**Alternative** (no auto-open):
```bash
xdg-open uat-guided.html  # Linux
open uat-guided.html      # macOS
```

### 3. After UAT execution

```bash
# Export JSON from the dashboard (button "Export JSON")
sddk uat ingest --session uat-session.json
sddk uat report --plan uat-plan.yaml
```

### 4. Acceptance gate

```
Critical path (P0) ≥ 5/5 → release-ready
High priority (P0+P1) ≥ 21/21 → usable
Zero unaddressed FAILs
Zero blocked-without-workaround
```

If gate passes, v1.0.0 tag is the next step (see ADR-031).

---

## Quick reference: what's where

| What | Where |
|------|-------|
| UAT plan (data) | `uat-plan.yaml` |
| Guided dashboard | `uat-guided.html` |
| Matrix dashboard | `uat-matrix.html` |
| UX critiques | `.impeccable/critique/*.md` |
| Visual deliverables | `docs/visual-deliverables/` |
| Original UAT prose | `docs/UAT-v1.0.0-cognicode.md` |
| Session memory | `mem_search cognicode` |

---

## Pending beyond UAT

1. **Mobile 320px** — still `test.fixme` (cosmetic, can be deferred)
2. **E30.4 carry-forwards** (W-1 to W-4) — don't block v1.0.0
3. **ADR-031 §4 denominator** — already applied (pct_verified over total - legacy_obsolete)
4. **CogniCodeExplorer-api ships with what?** — review ADR-031 §2 definition of done before tagging

---

## Last commit

```
c5006fb2 docs(impeccable): record UX reconnaissance v2 (score 29)
6ca7a0b1 feat(uat): v1.0.0 UAT plan with guided-mode dashboard
b4f889b0 polish(ui): Round-2 polish — 6 P0/P2 fixes from reconnaissance
0bca020c polish(ui): resolve all 6 P2/P3 carry-forward UX issues
350dbd65 docs(impeccable): record UX critique + post-fix snapshot
e58715e2 fix(ui): replace light-theme color leaks in dark-mode components
eb237e1d fix(ui): resolve 3 P0 UX issues from critique report
```

---

*Date: 2026-08-07 · Owner: Rubentxu · Status: ready for human UAT*
