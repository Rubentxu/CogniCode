# Proposal: Contextual Help ("What Can I Do Here?")

## Intent

Users inspecting objects (symbols, files, modules, etc.) have no guidance on what actions are available. Add per-object-type suggested questions ("What can I do here?") surfaced in the ObjectInspector, so users discover MCP capabilities without prior knowledge of tool names or graph requirements.

## Scope

### In Scope
- Static suggestion map: 9 `InspectableObjectType` variants → 3-5 prompts each
- Rendered inline in ObjectInspector as a suggestion strip between header and ViewTabs
- Prompt clicks dispatch `cognicode_ask` or direct MCP tools (e.g., `explorer_inspect_object`)
- Graph-unavailable detection: check `graph_status` before dispatching graph-dependent patterns
- Responsive collapse to popover on viewports < 900px
- CI check: verify static map matches `help-and-onboarding.md` content

### Out of Scope
- Backend changes (no new MCP tools, no API modifications)
- Dynamic/hotspot-aware suggestions (v2)
- Personalization or user-specific prompts
- Follow-up integration (existing `suggested_follow_ups` untouched)
- Internationalization

## Capabilities

### New Capabilities
- `contextual-help`: Per-object-type suggestion strip in ObjectInspector. Shows 3-5 action prompts based on the focused object's `InspectableObjectType`, dispatches relevant MCP tools on click.

### Modified Capabilities
- None

## Approach

Frontend-only static configuration map keyed by `InspectableObjectType`. A new `apps/explorer-ui/src/config/suggestedQuestions.ts` defines prompts and tool routing. ObjectInspector reads the focused object's type, renders suggestions, and uses a lightweight `useAsk` hook to dispatch. Zero backend changes. Graph-dependent patterns (6 of 8 in `cognicode_ask`) gate on `graph_status` availability.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/config/suggestedQuestions.ts` | New | Static prompt map (9 types, 3-5 prompts each) |
| `apps/explorer-ui/src/components/ObjectInspector/index.tsx` | Modified | Render suggestion strip |
| `apps/explorer-ui/src/state/context.ts` | Modified | Possible `useAsk` hook integration |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Content drift vs docs | Med | CI check comparing map to `help-and-onboarding.md` |
| Graph-unavailable dispatch failures | Low | Gate graph-dependent prompts on `graph_status` |
| UI clutter on small screens | Low | Collapse to popover < 900px |

## Rollback Plan

Remove the suggestion strip render from ObjectInspector and delete `suggestedQuestions.ts`. No backend state to revert.

## Dependencies

- `cognicode_ask` NL router (existing)
- `InspectableObjectType` enum in shared DTOs (existing)
- `explorer_inspect_object`, `explorer_get_view`, `explorer_open_workspace` (existing MCP tools)

## Success Criteria

- [ ] All 9 object types show 3-5 relevant suggestions in ObjectInspector
- [ ] Clicking a suggestion dispatches the correct MCP tool with appropriate params
- [ ] Graph-dependent prompts are hidden/disabled when `graph_status` is unavailable
- [ ] UI collapses to popover on viewports < 900px
