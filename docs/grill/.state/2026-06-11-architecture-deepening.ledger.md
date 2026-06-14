# Ledger — Architectural Deepening Candidates

## Pass 0: Initialization — 2026-06-11T19:37
| Cycle | Question ID | Candidate | Decision | Status |
|-------|-------------|-----------|----------|--------|
| — | — | — | — | Initializing |

## Pass 1 — 2026-06-11

### Q001-P1 — Dependency: C1 (Tool Registry) vs C4 (Schema/DTO)

**Question:** Can C1 begin independently of C4?

**Candidate:** 4 → 1 dependency | **Category:** dependency
**Status:** modified | **Confidence:** medium

**Proxy answer:** C1 can begin independently. Recommended ordering: C1→C4. Confidence: high.

**Skeptic challenge:** Boundary is illusory. Evidence: schemas.rs imports application::dto, handle_get_hot_symbols returns a DTO, BuildGraphInput has 53 refs in handlers/mod.rs, dto_mapping.rs is dead code.

**Judge final answer:** C1 can begin before C4 ONLY after three preconditions: (1) Move BuildGraphInput to schemas.rs, (2) Audit all handler return types for DTO leakage, (3) Remove schemas.rs's dependency on application::dto. OR alternatively: run C4 first, then C1.

**Rejection trace:**
- Rejection reason: Proxy assumed clean boundary without code verification
- Remedy: `precondition_gate`
- What Proxy missed: Did not verify imports, did not trace handler return types

**Impact:** CONTEXT.md (boundary rules), ADR candidate (schema-dto-boundary), CI audit tooling needed

**Follow-up:** 4 questions generated (CI check, BuildGraphInput ownership, audit ownership, C4 prerequisite scope)
