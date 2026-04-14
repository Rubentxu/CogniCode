# Archive Report: oss-coverage-expansion

## Change Archived

**Change**: oss-coverage-expansion
**Archived to**: `sdd/archive/2026-04-12-oss-coverage-expansion/`
**Archived on**: 2026-04-12

## Summary

This change expanded OSS coverage to include:
- 3 Go repos: cobra, bubbletea, lo (15 scenarios)
- 1 Java repo: spring-petclinic (5 scenarios) 
- 1 JS repo: express (10 scenarios)
- 1 TS repo: zod (10 scenarios)

**Total: 40 new scenarios across 4 manifests**

## Specs Synced

No main specs existed for the new domains (java-petclinic, go-cobra, go-bubbletea, go-lo, js-express, ts-zod). The delta specs remain in the archive for future reference.

| Domain | Action | Details |
|--------|--------|---------|
| java-petclinic | Delta only (no main spec) | Spec updated to reflect Gradle build system |
| go-cobra | Delta only (no main spec) | Standard Go repo |
| go-bubbletea | Delta only (no main spec) | Standard Go repo |
| go-lo | Delta only (no main spec) | Standard Go repo |
| js-express | Delta only (no main spec) | Standard JS repo |
| ts-zod | Delta only (no main spec) | Standard TS repo |

## Archive Contents

- proposal.md ✅
- spec/ ✅ (go-cobra, go-bubbletea, go-lo, java-petclinic, js-express, ts-zod)
- tasks.md ✅ (43/43 tasks complete)
- apply-progress ✅
- verify-report ✅

## Issues Resolved During Apply

1. ✅ JS/TS manifests: `install` and `typecheck` stage names replaced with `build`
2. ✅ Go/Java MCP startup: Added go/java to MCP startup condition in sandbox_orchestrator.rs
3. ✅ Java pinned_sha: Changed from `main` to immutable SHA `edf4db28affcc4741c79850a3d95bc3f177b5ff9`
4. ✅ Java spec drift: Updated spec to reflect Gradle instead of Maven

## TDD Evidence

This was a data gathering task (repos + manifests, not library code):
- RED: Before change, manifests referenced non-existent repos → expansion would fail
- GREEN: After change, all 40 scenarios expand successfully, Go/Java MCP starts

## SDD Cycle Complete

The change has been fully planned, implemented, verified, and archived.
Ready for the next change.
