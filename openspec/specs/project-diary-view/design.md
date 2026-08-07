# Design: ProjectDiary ViewExecutor

## Technical Approach

Implement a `ProjectDiaryExecutor` following the `ComposedNarrativeExecutor` pattern — a thin async wrapper over a pure synchronous shaper. The executor receives a pre-resolved `InspectionTarget::Workspace` (carrying workspace id + exploration sessions) and delegates to `build_project_diary`, which produces a `ContextualView` with one `ViewBlock` per session.

## Architecture Decisions

### Decision: InspectionTarget::Workspace variant

**Choice**: Add `InspectionTarget::Workspace(WorkspaceTarget)` enum variant
**Alternatives considered**: Pass only `workspace_id` and fetch sessions inside the executor via async port
**Rationale**: The `ComposedNarrative` pattern passes fully-resolved data to shapers. The shaper must be pure/synchronous. Async session loading happens in the service layer before `build()` is called, matching how `SavedExploration` is resolved (facades/view.rs:374-395).

### Decision: Block model for narrative entries

**Choice**: Implicit block types via `body.block_type` discriminator
**Alternatives considered**: New enum `DiaryBlockKind` with dedicated structs
**Rationale**: `ViewBlock.body` is already `serde_json::Value`. Adding a `block_type` string discriminator avoids a new enum + serialisation boilerplate. Frontend switches on `body.block_type` to pick the renderer. This matches how `build_investigation_narrative` works (views.rs:4703-4711 for narrative, 4716-4731 for evidence).

### Decision: Session data shape

**Choice**: `WorkspaceTarget { workspace_id: String, sessions: Vec<ExplorationSession> }`
**Alternatives considered**: Only pass workspace_id, fetch sessions in shaper via sync port
**Rationale**: Sessions are already loaded by the service layer (SessionStore). Passing them pre-resolved keeps the shaper pure and deterministic per spec requirement "Shaper is deterministic".

## Data Flow

```
Service resolves workspace + sessions
         │
         ▼
InspectionTarget::Workspace(WorkspaceTarget)
         │
         ▼
ProjectDiaryExecutor::build(ctx)
         │
         ▼
build_project_diary(workspace_id, &sessions)   ← pure shaper
         │
         ▼
ContextualView { blocks: Vec<ViewBlock>, view_kind: ProjectDiary, ... }
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modify | Add `WorkspaceTarget` struct + `InspectionTarget::Workspace` variant |
| `crates/cognicode-explorer/src/domain/views.rs` | Modify | Add `ProjectDiaryExecutor` + `build_project_diary` shaper |
| `crates/cognicode-explorer/src/registry.rs` | Modify | Register `PROJECT_DIARY_EXECUTOR` in `REAL_EXECUTORS` map |

## Interfaces / Contracts

```rust
// dto.rs — new types
pub struct WorkspaceTarget {
    pub workspace_id: String,
    pub sessions: Vec<ExplorationSession>,
}

// views.rs — executor impl
pub struct ProjectDiaryExecutor;
impl ViewDescriptor for ProjectDiaryExecutor {
    fn id(&self) -> &'static str { "project-diary" }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Workspace]
    }
    fn view_kind(&self) -> ViewKind { ViewKind::ProjectDiary }
    fn renderer_kind(&self) -> RendererKind { RendererKind::Composite }
}
impl ViewExecutor for ProjectDiaryExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Workspace(wt) => Ok(build_project_diary(&wt.workspace_id, &wt.sessions)),
            _ => Err(ExplorerError::ViewNotAvailable { ... }),
        }
    }
}

// Pure shaper (no async, no port access)
pub fn build_project_diary(workspace_id: &str, sessions: &[ExplorationSession]) -> ContextualView {
    let blocks: Vec<ViewBlock> = if sessions.is_empty() {
        vec![ViewBlock { id: "empty".into(), title: "No sessions".into(),
            body: json!({ "block_type": "placeholder", "message": "No exploration sessions" }) }]
    } else {
        sessions.iter().enumerate().map(|(i, s)| ViewBlock {
            id: format!("session:{}", i),
            title: s.id.clone(),
            body: json!({
                "block_type": "session",
                "session_id": s.id,
                "event_count": s.events.len(),
                "created_at": s.created_at,
                "investigation_id": s.investigation_id,
            }),
        }).collect()
    };
    ContextualView { object_id: workspace_id.into(), view_id: "project-diary".into(),
        title: "Project Diary".into(), view_kind: ViewKind::ProjectDiary,
        renderer_kind: RendererKind::Composite, blocks, relations: vec![],
        evidence: vec![], findings: vec![] }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `build_project_diary` with 0/1/many sessions | Direct shaper call with fixtures |
| Unit | `ProjectDiaryExecutor::build` dispatches correctly | Mock `ViewContext` with `Workspace` target |
| Integration | Registry lists `project-diary` for `InspectableObjectType::Workspace` | `list_for(Workspace)` assertion |

## Open Questions

- [ ] Should `build_project_diary` include the narrative content from linked investigations, or only session metadata?
- [ ] Does the frontend need a `RendererKind::ProjectDiary` variant, or does `Composite` suffice?
