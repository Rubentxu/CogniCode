# Archive Manifest — Bulk historical pre-archive-manifest cycles

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/bulk-historical-pre-m13` |
| Path | housekeeping |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-bulk-historical`** |
| Superseded at | 2026-09-21T06:48Z |

## Scope

This archive-manifest covers 43 change directories that predate the
archive-manifest formalism (introduced 2026-09-15 with e51's cycle).
All 43 changes:

1. Have their product commits already on `main` (verified via
   `git log --oneline -- <change-dir>`).
2. Were already considered "closed" by their respective owners
   at the time of merge.
3. Did not have a formal `archive-manifest.md` in their directory
   because the formalism did not yet exist.
4. Have only proposal/spec/tasks/design files (no live active state,
   no `verify-report.md` left open).

The 43 changes are:

| Change | Type |
|---|---|
| cognicode-living-software-intelligence | umbrella |
| e11-truncation-field-naming | housekeeping |
| e12b-api-surface | foundation |
| e12c-test-slice | foundation |
| e12f-ownership-map | foundation |
| e12g-risk-map | foundation |
| e12h-decision-trace | foundation |
| e12-viewkind-realization | foundation |
| e14-narrative-runtime | narrative |
| e14-narrative-runtime-cycle-2 | narrative |
| e17-e2e-coverage-audit | test |
| e19-3-c4-overlays | architecture |
| e19-5-expected-architecture | architecture |
| e20-1-mermaid-c4-export | architecture |
| e20-3-drawio-action | architecture |
| e20-4-svg-snapshot | architecture |
| e25-decision-support-packs | foundation |
| e28-0-canonical-graph-revisions | graph |
| e28-1-moldplan-graphplan-contracts | graph |
| e28-2-differential-graph-executors | graph |
| e28-2-runtime-closure | graph |
| e28-3-moldql-pattern-profile-v1 | graph |
| e28-4-analytics-registry-cohort-1 | graph |
| e28-5-structural-analytics-cohort-2 | graph |
| e28-6-advanced-analytics-evidence-gate | graph |
| e29-0-trustworthy-delivery-baseline | runtime |
| e29-1-temporal-graph-and-atomic-ingest | runtime |
| e29-2-semantic-projection-kernel | runtime |
| e29-3-moldable-explorer-runtime | runtime |
| e29-3-port-abstraction-audit | runtime |
| e29-4-scale-operability-proof | runtime |
| e38.1-lsi-debt-hardening | lsi |
| e38.2-lsi-preflight | lsi |
| e40-lsi-generic-graph-equivalence-harness | lsi |
| e79-1-canary-characterize-debt-002 | lsi |
| e79-1-trust-baseline-hardening | lsi |
| e84-cognicode-distribution-artifact-contract | umbrella |
| e9-landing-perf | housekeeping |
| m5-1-ast-lifting | m5 |
| m5-1b-debt-cleanup | m5 |
| m5-1b-statement-extraction | m5 |
| m5-mcp-wiring | m5 |
| m5-program-analysis-core | m5 |

## Acceptance verdicts (aggregate)

| Acceptance criterion | Status | Evidence |
|---|---|---|
| All 43 changes have product on main | ✅ PASS | `git log --oneline -- <each-change-dir>` shows merge commits |
| None have open verify-report | ✅ PASS | No `verify-report.md` outside of those already archived |
| None have live state (pending .yaml) | ✅ PASS | Reviewed during pre-move inspection |
| All were "closed by their owner" | ✅ PASS | Per LSI umbrella closure (M0..M9 closed) and historical roadmap reconciliation |

## Note on e84

`e84-cognicode-distribution-artifact-contract` is the umbrella for the
distribution contract work. Its successor cycles (e86.*, e87, e88) are
already archived. This archive entry closes the umbrella itself; the
distribution contract is published in `openspec/specs/cognicode-distribution/`.

## Note on cognicode-living-software-intelligence

This is the LSI umbrella itself, not a single cycle. Its product lives
in the entire LSI roadmap (M0..M14), with all product milestones on
`main`. This archive entry supersedes the umbrella dir; the operational
status remains in `docs/ROADMAP.md` (local-only).

## Note on m5-*

The m5-* family is the M5 (Program Analysis) milestone of the original
roadmap. All product shipped in M5's milestones is on `main`. This
archive entry closes the m5-* change dirs; the spec lives in
`openspec/specs/cognicode-program-analysis/`.

## Closure semantics

```text
implementation      CLOSED (each merge commit on main)
verification       historical (no verify-report per change; aggregate
                   evidence is the LSI umbrella closure)
archive closure    DONE   (this manifest)
```
