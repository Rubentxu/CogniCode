# CogniCode Improvement Plan - Agent Team Setup

> **Created**: 2026-03-29
> **Status**: ✅ PLAN COMPLETE
> **Reference**: `openspec/changes/improvement-plan-2026-Mar-29/proposal.md`

## Team Structure

### Coordinator
- **Role**: Orchestrates phase execution, monitors progress, validates criteria
- **Reports to**: (none - top level)

### Phase Teams

#### Phase 0: Stabilization Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Fix tests, remove duplicates, clean warnings |
| Reviewer | rust-idiomatic-expert | Verify idiomatic Rust patterns |

#### Phase 1: Walking Skeleton Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | get_file_symbols end-to-end |
| Reviewer | architecture-critic | Validate vertical slice architecture |

#### Phase 2: Real Graph Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Call graph, impact analysis |
| Reviewer | balance-advisor | Verify coupling balance |

#### Phase 3: Real Refactoring Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Rename strategy, SafetyGate |
| Reviewer | rust-idiomatic-expert | Verify safe refactoring patterns |

#### Phase 4: Maturity Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | E2E tests, VFS, tree-sitter upgrade |
| Reviewer | architecture-critic | Validate overall architecture |

#### Phase 5: Diferenciación Team
| Role | Agent | Responsibilities |
|------|-------|------------------|
| Lead | sdd-apply | Context compression, incremental graph, LSP proxy |
| Reviewer | architecture-critic | Validate competitive differentiation |

## Phase Dependencies

```
Phase 0 ──► Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4 ──► Phase 5
              │
              └── (parallel review by architecture-critic)
```

## Definition of Done

Each phase MUST meet these criteria before proceeding:

| Phase | Criteria |
|-------|----------|
| 0 | `cargo test` passes, `cargo check` 0 errors |
| 1 | `get_file_symbols` returns real symbols from test files |
| 2 | Call graph built, `get_call_hierarchy` returns real data |
| 3 | Rename generates real TextEdits, SafetyGate validates |
| 4 | 120+ tests pass, tree-sitter 0.24+, VFS integrated |
| 5 | Context compression, incremental graph, LSP proxy |
| 5.2 | Deepen Phase 5 (tests, integration) |

## Communication Protocol

1. Phase Lead creates tasks via `sdd-tasks`
2. Phase Lead implements via `sdd-apply`
3. Phase Lead verifies via `sdd-verify`
4. Reviewer validates patterns and architecture
5. Coordinator monitors and approves phase completion

## Git Tagging

After each phase completion:
- `v0.1-stabilization`
- `v0.2-walking-skeleton`
- `v0.3-real-graph`
- `v0.4-real-refactor`
- `v0.5-mvp`

## Current Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| 0 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 1 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 2 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 3 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 4 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 5 | ✅ COMPLETE | 2026-03-29 | 2026-03-29 |
| 5.2 | ✅ COMPLETE | 2026-03-30 | 2026-03-30 |

## Active Delegations

| ID | Agent | Task |
|----|-------|------|
| (none) | - | All phases complete |

## Phase 5 Features

| Feature | Description |
|---------|-------------|
| Context Compression | Transform JSON responses to natural language summaries for AI agents |
| Incremental Graph Updates | Update graph incrementally instead of rebuild-every-time |
| LSP Proxy Mode | Connect to rust-analyzer/pyright, add CogniCode premium intelligence |

## Completed Artifacts

| File | Description |
|------|-------------|
| `openspec/config.yaml` | SDD project config |
| `openspec/changes/improvement-plan-2026-Mar-29/proposal.md` | Full change proposal |
| `openspec/changes/improvement-plan-2026-Mar-29/team-setup.md` | Team structure and phase tracking |
