# Skill Registry — CogniCode

Generated: 2026-06-07
Skills indexed: 55 (3 scopes)

## Registry Contract
- SKILL.md is the source of truth — this file is an index only
- Sub-agents receive skill paths, not summaries
- Deduplication: project-level skills preferred over user-level

## Skills by Scope

### Project-Level (.claude/skills/)
| Name | Trigger | Path |
|------|---------|------|
| bug-find | debug, bug, crash | `.claude/skills/bug-find/SKILL.md` |
| rust-ddd-expert | DDD, hexagonal, domain | `.claude/skills/rust-ddd-expert/SKILL.md` |
| ralph-rust | Rust patterns, idioms | `.claude/skills/ralph-rust/SKILL.md` |
| pretty-mermaid | render mermaid, flowchart, diagram | `.claude/skills/pretty-mermaid/SKILL.md` |
| documentacion | docs, documentation | `.claude/skills/documentacion/SKILL.md` |
| product-owner | PO, backlog, prioritization | `.claude/skills/product-owner/SKILL.md` |
| review-wasm | WASM audit, ECS, performance | `.claude/skills/review-wasm/SKILL.md` |
| frontend-design | UI, frontend, React, components | `.claude/skills/frontend-design/SKILL.md` |
| pruebas-cli | CLI testing, integration tests | `.claude/skills/pruebas-cli/SKILL.md` |
| investigacion | research, investigation | `.claude/skills/investigacion/SKILL.md` |
| git-versioning | git, versioning, branching | `.claude/skills/git-versioning/SKILL.md` |
| refactor | refactor, restructure | `.claude/skills/refactor/SKILL.md` |
| doc-writer | docs writing, README, guides | `.claude/skills/doc-writer/SKILL.md` |

### User-Level (opencode config)
| Name | Trigger | Path |
|------|---------|------|
| auto-grill | challenge plans/proposals/designs | `~/.config/opencode/skills/auto-grill/SKILL.md` |
| auto-grill-loop | /auto-grill-loop, exhaustive questioning | `~/.config/opencode/skills/auto-grill-loop/SKILL.md` |
| entropy-sdd | entropy, connascence, DQS, SOLID | `~/.config/opencode/skills/entropy-sdd/SKILL.md` |
| cognicode-sdd | CogniCode analysis, impact, refactoring | `~/.config/opencode/skills/cognicode-sdd/SKILL.md` |
| chronos-mcp | debug, crash, trace, time-travel | `~/.config/opencode/skills/chronos-mcp/SKILL.md` |
| chronos-sdd | runtime evidence, regression | `~/.config/opencode/skills/chronos-sdd/SKILL.md` |
| rust-patterns | Rust API design, generics, async | `~/.config/opencode/skills/rust-patterns/SKILL.md` |
| branch-pr | PR creation, pull request | `~/.config/opencode/skills/branch-pr/SKILL.md` |
| chained-pr | stacked PRs, review slices | `~/.config/opencode/skills/chained-pr/SKILL.md` |
| issue-creation | GitHub issue, bug report | `~/.config/opencode/skills/issue-creation/SKILL.md` |
| work-unit-commits | commit planning, atomic commits | `~/.config/opencode/skills/work-unit-commits/SKILL.md` |
| test-pyramid | testing strategy, test design | `~/.config/opencode/skill/test-pyramid/SKILL.md` |
| playwright-cli | browser automation, Playwright | `~/.config/opencode/skills/playwright-cli/SKILL.md` |
| comment-writer | PR comments, review feedback | `~/.config/opencode/skills/comment-writer/SKILL.md` |
| cognitive-doc-design | docs, README, RFC, architecture | `~/.config/opencode/skills/cognitive-doc-design/SKILL.md` |
| frontend-evidence-loop | fix frontend, UI bug, visual regression | `~/.config/opencode/skills/frontend-evidence-loop/SKILL.md` |
| layout-geometry-audit | layout issue, alignment, overflow | `~/.config/opencode/skills/layout-geometry-audit/SKILL.md` |
| ui-audit-protocol | audit UI, visual QA, browser verification | `~/.config/opencode/skills/ui-audit-protocol/SKILL.md` |
| go-testing | Go tests, teatest, golden files | `~/.config/opencode/skills/go-testing/SKILL.md` |
| judgment-day | dual review, adversarial review | `~/.config/opencode/skills/judgment-day/SKILL.md` |
| skill-creator | create skills, agent instructions | `~/.config/opencode/skills/skill-creator/SKILL.md` |
| skill-improver | improve skills, audit skills | `~/.config/opencode/skills/skill-improver/SKILL.md` |
| logseq-vault | LogSeq vault, SDD artifacts | `~/.config/opencode/skills/logseq-vault/SKILL.md` |
| minimax-mcp | web search, image analysis | `~/.config/opencode/skills/minimax-mcp/SKILL.md` |
| zai-mcp | web search fallback, z.ai | `~/.config/opencode/skills/zai-mcp/SKILL.md` |

### User-Level (.agents/skills/)
| Name | Trigger | Path |
|------|---------|------|
| accessibility | WCAG, a11y audit, screen reader | `~/.agents/skills/accessibility/SKILL.md` |
| best-practices | security audit, modernize code | `~/.agents/skills/best-practices/SKILL.md` |
| core-web-vitals | LCP, INP, CLS, page speed | `~/.agents/skills/core-web-vitals/SKILL.md` |
| design-an-interface | design API, interface options | `~/.agents/skills/design-an-interface/SKILL.md` |
| design-md | DESIGN.md, design system | `~/.agents/skills/design-md/SKILL.md` |
| diagnose | debug, diagnose, broken, failing | `~/.agents/skills/diagnose/SKILL.md` |
| find-skills | discover skills, install skills | `~/.agents/skills/find-skills/SKILL.md` |
| frontend-design | web components, React, dashboard | `~/.agents/skills/frontend-design/SKILL.md` |
| grill-me | stress-test plan, get grilled | `~/.agents/skills/grill-me/SKILL.md` |
| grill-with-docs | grill against docs, ADRs, CONTEXT.md | `~/.agents/skills/grill-with-docs/SKILL.md` |
| improve-codebase-architecture | refactoring, architecture improvement | `~/.agents/skills/improve-codebase-architecture/SKILL.md` |
| leptos-guide | Leptos v0.8, signals, components | `~/.agents/skills/leptos-guide/SKILL.md` |
| mmx-cli | MiniMax CLI, media generation | `~/.agents/skills/mmx-cli/SKILL.md` |
| performance | speed up, optimize, load time | `~/.agents/skills/performance/SKILL.md` |
| playwright-best-practices | Playwright tests, E2E, flaky tests | `~/.agents/skills/playwright-best-practices/SKILL.md` |
| teach | teach concept, learn | `~/.agents/skills/teach/SKILL.md` |
| web-quality-audit | audit site, lighthouse, quality | `~/.agents/skills/web-quality-audit/SKILL.md` |
| webapp-testing | test web app, Playwright, browser | `~/.agents/skills/webapp-testing/SKILL.md` |

## Compact Rules (pre-resolved for SDD phases)

### For sdd-explore / sdd-propose / sdd-design
- entropy-sdd: Connascence landscape mandatory (Protocol A). Entropy budget mandatory (Protocol B). Information Bottleneck check mandatory (Protocol C). DQS = f(S, D, I, R, C). Threshold: I < 3.0 bits.
- auto-grill: Challenge proposals/designs against codebase. Escalate to user when AES >= 0.5.
- cognicode-sdd: Use cognicode_* tools for codebase analysis. Prefer over raw file reads.

### For sdd-apply
- Correction mode: only fix CRITICAL findings, no new features.
- Apply-progress: read-merge-write on continuation batches.
- Strict TDD: write test first, then implementation.

### For sdd-verify
- entropy-sdd Protocol D: Design Quality Score + SOLID-Entropy compliance.
- Adversarial judgment: 2 blind judges, AES scoring, max 2 correction iterations.
- Quality gate: ALL findings AES < 0.25 → PASS. ANY >= 0.5 → FAIL.
- cognicode-quality: use get_quality_diff for quality comparison.

### For sdd-archive
- entropy-sdd Protocol E: entropy trend across changes.
- Sync final state to artifact store.
