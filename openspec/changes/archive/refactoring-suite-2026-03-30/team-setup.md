# Refactoring Suite - Agent Team Setup

> **Created**: 2026-03-30
> **Status**: Active
> **Reference**: `openspec/changes/refactoring-suite-2026-03-30/proposal.md`

## Team Structure

### Team Lead (Coordinator)
- Orchestrates implementation across all 4 refactoring features

### Feature Teams (Parallel)

#### Team 1: Extract Method
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Implement ExtractStrategy, tests |

#### Team 2: Inline Method
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Implement InlineStrategy, tests |

#### Team 3: Move Symbol
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Implement MoveStrategy, tests |

#### Team 4: Change Signature
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Implement ChangeSignatureStrategy, tests |

## Definition of Done (Each Feature)

| Feature | Criteria |
|---------|----------|
| Extract | Can extract 5+ line block into new function, updates call site |
| Inline | Can replace call with method body, remove original if unused |
| Move | Can move symbol to different file, update imports |
| ChangeSignature | Can rename/add/remove parameters, update all call sites |

## Implementation Pattern

Each strategy file follows the same structure:
```
src/infrastructure/refactor/
├── rename_strategy.rs      (existing)
├── extract_strategy.rs      (NEW)
├── inline_strategy.rs       (NEW)
├── move_strategy.rs         (NEW)
└── change_signature_strategy.rs  (NEW)
```

Each implements:
1. `validate()` - Check preconditions
2. `prepare_edits()` - Generate TextEdits
3. `preview()` - Return changes without applying

## Current Status

| Feature | Status | Tests |
|---------|--------|-------|
| Extract | ✅ Implemented | Passing |
| Inline | ✅ Implemented | Passing |
| Move | ✅ Implemented | Passing |
| ChangeSignature | ✅ Implemented | Passing |

**Overall**: 210 tests total, 1 failing (pre-existing bug in `impact_analyzer`)

## Remaining Issue

The 1 failing test (`test_impact_analyzer_with_dependents`) is a pre-existing bug in `find_all_dependents` fallback name-based search that incorrectly finds extra symbols. This is tracked separately from the refactoring suite implementation.

## Active Delegations

All delegations completed. Implementation phase finished.
