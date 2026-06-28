# SDDK Workflow — Moldable UX + C4 Investigation + Diagram Representations

**Workflow type**: A-full (architectural, multi-milestone program)
**ADRs**: ADR-003 (diagrams), ADR-004 (C4), ADR-005 (investigations)
**Execution mode**: interactive (user reviews each phase)
**Artifact store**: engram + local docs/

---

## Triage

```
Context quality: C1 (codebase well-known, architecture clear from ADR-002)
Jurisprudence hits: ADR-002 (moldable exploration parity), CONTEXT.md (full domain vocabulary)
Path: A-full (architectural program, 4 milestones, new entities + new UI surfaces)
Capabilities deployed:
  - CogniCode-sdd: YES (architectural analysis, impact assessment)
  - Entropy-sdd: YES (effort ≥ deepen)
  - Web search: NO (no external research needed)
  - impeccable: YES (frontend design — landing workbench, investigation sidebar, C4 UI)
  - Multi-lens verify: YES (A-full path)
F3 tuning: ON (first cycle of this program)
```

---

## Phase sequence per milestone

### E18 — Moldable UX Foundation

| Phase | Agent | Input | Output | Gate |
|-------|-------|-------|--------|------|
| explore | sddk-explore | Current UX flows (ShellLayout, Spotter, PaneStackView, PaneInspector, SuggestionStrip) | UX audit report: what works, what's missing, journey gaps | approved |
| propose | sddk-propose | UX audit + ADR-005 | Proposal: landing workbench, spotter intent, causal breadcrumbs, suggestion verbs | approved |
| spec | sddk-spec | Proposal | Delta specs: Given/When/Then for each UX change | approved |
| design | sddk-design | Proposal + codebase | Component design: new components, state changes, API changes | ≥ 60 coherence |
| tasks | sddk-tasks | Spec + design | Implementation tasks per UX change | approved |
| apply | sddk-apply | Tasks | Committed code | git-boundary lint pass |
| verify | sddk-verify | Specs + code | Verify report: PASS or PW | PASS/PW |
| archive | sddk-archive | Verify report | Archive report, ROADMAP update | done |

### E19 — C4 Investigation Model

| Phase | Agent | Input | Output | Gate |
|-------|-------|-------|--------|------|
| explore | sddk-explore | Current C4 implementation (architecture_handler, build_architecture, PerspectiveToggle, useArchitecture) + ADR-004 | C4 gap report: what's directory-derived vs semantic | approved |
| propose | sddk-propose | C4 gap + ADR-004 | Proposal: rename, level selector, overlays, dynamic views, expected architecture | approved |
| spec | sddk-spec | Proposal | Delta specs for each C4 level | approved |
| design | sddk-design | Proposal + backend code | Backend design: new ViewExecutors for C4 levels, overlay data sources, drift comparison | ≥ 60 coherence |
| tasks | sddk-tasks | Spec + design | Tasks: backend executors + frontend selector + overlays | approved |
| apply | sddk-apply | Tasks | Committed code | git-boundary lint pass |
| verify | sddk-verify | Specs + code | Verify report | PASS/PW |
| archive | sddk-archive | Verify report | Archive report | done |

### E20 — Diagram Representations

| Phase | Agent | Input | Output | Gate |
|-------|-------|-------|--------|------|
| explore | sddk-explore | Current Mermaid export (export_mermaid, to_mermaid) + draw.io research + ADR-003 | Diagram export gap report | approved |
| propose | sddk-propose | Gap + ADR-003 | Proposal: Mermaid C4 export, draw.io action, SVG snapshot | approved |
| spec | sddk-spec | Proposal | Delta specs for export endpoints + UI actions | approved |
| design | sddk-design | Proposal + backend code | Design: `to_mermaid()` per ViewKind, export API endpoint, frontend action wiring | ≥ 60 coherence |
| tasks | sddk-tasks | Spec + design | Tasks: Mermaid generators + API + UI actions | approved |
| apply | sddk-apply | Tasks | Committed code | git-boundary lint pass |
| verify | sddk-verify | Specs + code | Verify report | PASS/PW |
| archive | sddk-archive | Verify report | Archive report | done |

### E21 — Investigation Mode

| Phase | Agent | Input | Output | Gate |
|-------|-------|-------|--------|------|
| explore | sddk-explore | ExplorationSession, ShareExplorationButton, ViewSpecStore + ADR-005 | Investigation gap report | approved |
| propose | sddk-propose | Gap + ADR-005 | Proposal: entity, API, sidebar, pin evidence, evidence pack, narrative | approved |
| spec | sddk-spec | Proposal | Delta specs for investigation lifecycle | approved |
| design | sddk-design | Proposal + backend + frontend | Design: PG schema, API routes, InvestigationSidebar component, EvidencePack executor | ≥ 60 coherence |
| tasks | sddk-tasks | Spec + design | Tasks: schema + API + UI + executors | approved |
| apply | sddk-apply | Tasks | Committed code | git-boundary lint pass |
| verify | sddk-verify | Specs + code | Verify report | PASS/PW |
| archive | sddk-archive | Verify report | Archive report | done |

---

## Dependency graph

```
E18 (UX foundation)
  ├──→ E20 (diagrams) ──→ E21 (investigations)
  └──→ E19 (C4) ──────────→ E20 (diagrams depend on C4 levels)
                             └──→ E21 (investigations embed diagrams)
```

**Parallel opportunities**:
- E18 + E19 can start simultaneously (independent surfaces)
- E20-1 (Mermaid C4) can start as soon as E19-2 (level selector) is designed
- E21-1 (entity + API) can start as soon as E18-1 (landing) is in apply

---

## Git strategy

| Milestone | Branch pattern | Merge strategy |
|-----------|---------------|----------------|
| E18 | `feat/e18-moldable-ux` | Squash-merge per change |
| E19 | `feat/e19-c4-investigation` | Squash-merge per change |
| E20 | `feat/e20-diagram-representations` | Squash-merge per change |
| E21 | `feat/e21-investigation-mode` | Squash-merge per change |

One branch per milestone. Multiple commits per branch (atomic per task).
Squash-merge to main when milestone verify passes.

---

## Capability injections per phase

| Phase | impeccable | cognicode-sdd | entropy-sdd | web search |
|-------|-----------|---------------|-------------|------------|
| E18 explore | audit current UX | analyze component dependencies | — | — |
| E18 design | shape landing + sidebar | — | connascence check | — |
| E18 apply | craft UI components | — | — | — |
| E19 explore | — | analyze C4 backend gaps | — | — |
| E19 design | C4 selector UI | — | entropy budget | — |
| E20 explore | — | — | — | draw.io docs (already done) |
| E20 design | export action UI | — | — | — |
| E21 explore | — | analyze exploration session | — | — |
| E21 design | investigation sidebar UI | — | — | — |

---

## Verify lenses (A-full multi-lens)

For each milestone's verify phase, launch 6 parallel lenses + 1 synthesis:

1. **Spec compliance** — does implementation match Given/When/Then specs?
2. **Architecture + connascence** — are new components properly decoupled?
3. **Test quality** — are tests meaningful, not just coverage?
4. **Design coherence** — does the UX flow make sense end-to-end?
5. **Adversarial Judge A** — blind review for bugs, edge cases, UX dead-ends
6. **Adversarial Judge B** — blind review for architecture, naming, maintainability
7. **Synthesis** — merge all lens reports, emit PASS / PW / FAIL verdict

---

## Model assignments

| Phase | Model |
|-------|-------|
| orchestrator | GLM-5.2 (current) |
| sddk-explore | GLM-5.1 |
| sddk-propose | DeepSeek V4 Pro |
| sddk-spec | DeepSeek V4 Pro |
| sddk-design | GLM-5.2 |
| sddk-tasks | GLM-5.2 |
| sddk-apply | GLM-5.2 |
| sddk-verify (lenses) | GLM-4.7 |
| sddk-verify (synthesis) | GLM-4.7 |
| sddk-archive | GLM-4.7 |

---

## Success criteria

The program is complete when:

1. ✅ Landing page offers structured entry points (not just a graph)
2. ✅ Spotter offers intent actions (open as call graph, C4, add to investigation)
3. ✅ Pane stack shows causal breadcrumbs
4. ✅ C4 has multiple levels (not just directory toggle)
5. ✅ C4 overlays show drift + hotspots
6. ✅ "Open in draw.io" works from C4 views and investigation panes
7. ✅ Mermaid C4 export works for context/container/component
8. ✅ Investigation entity exists with pin evidence + narrative + artifacts
9. ✅ Evidence Pack and Composed Narrative views are wired
10. ✅ 127+ E2E tests still passing (no regressions)
