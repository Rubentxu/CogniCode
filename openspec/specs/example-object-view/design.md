# Design: ExampleObject ViewExecutor

## Technical Approach

Implement an `ExampleObjectExecutor` following the `ComposedNarrative` pattern — a thin async wrapper over a pure synchronous shaper. The executor receives a pre-resolved `InspectionTarget::Symbol` and resolves examples via `ctx.graph_repo` before calling the shaper. The shaper produces a `ContextualView` with one `ViewBlock` per example (code + metadata).

## Architecture Decisions

### Decision: Example data comes from graph repository

**Choice**: Resolve examples via `ctx.graph_repo` before calling the shaper
**Alternatives considered**: Examples embedded directly in `ResolvedSymbol`
**Rationale**: Examples are first-class graph nodes (Doc kind with `example_of` relations). The graph repository already has query capabilities. The shaper must be pure, so async resolution happens in the executor before shaper dispatch. This mirrors how `EvidenceExecutor` uses `graph_repo` to fetch evidence nodes.

### Decision: ExampleBlock struct for shaper input

**Choice**: Define `ExampleBlock { code, language, file, line, example_id }` as shaper input
**Alternatives considered**: Pass raw JSON from graph repo directly to shaper
**Rationale**: The shaper should receive typed, validated data. `ExampleBlock` is a domain struct that the executor populates from graph query results. The shaper transforms `ExampleBlock` → `ViewBlock`. This separation of concerns (resolve vs shape) matches the existing pattern in `build_investigation_narrative` where evidence is resolved before being shaped.

### Decision: Block body shape for examples

**Choice**: `body.block_type === "example"` with `code`, `language`, `file`, `line` fields
**Alternatives considered**: Separate `ExampleBlock` enum variant in `ViewBlock`
**Rationale**: `ViewBlock.body` is `serde_json::Value`. Adding a `block_type` discriminator avoids a new enum and keeps serialization simple. Frontend renders via `switch(body.block_type)`. This is consistent with how `ProjectDiary` blocks work.

## Data Flow

```
Symbol target resolved by service layer
         │
         ▼
ExampleObjectExecutor::build(ctx)
         │
         ├─ ctx.graph_repo.example_blocks_for_symbol(symbol.id)?  ← async resolution
         │
         ▼
build_example_object(symbol, &examples)  ← pure shaper
         │
         ▼
ContextualView { blocks: Vec<ViewBlock>, view_kind: ExampleObject, ... }
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/domain/views.rs` | Modify | Add `ExampleBlock` struct + `ExampleObjectExecutor` + `build_example_object` shaper |
| `crates/cognicode-explorer/src/registry.rs` | Modify | Register `EXAMPLE_OBJECT_EXECUTOR` in `REAL_EXECUTORS` map |

## Interfaces / Contracts

```rust
// views.rs — new domain struct (shaper input)
#[derive(Debug, Clone)]
pub struct ExampleBlock {
    pub example_id: String,
    pub code: String,
    pub language: String,
    pub file: String,
    pub line: u32,
    pub description: Option<String>,
}

// Executor impl
pub struct ExampleObjectExecutor;
impl ViewDescriptor for ExampleObjectExecutor {
    fn id(&self) -> &'static str { "example-object" }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind { ViewKind::ExampleObject }
    fn renderer_kind(&self) -> RendererKind { RendererKind::Composite }
}
impl ViewExecutor for ExampleObjectExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => {
                // Resolve examples from graph repo (async) then call pure shaper
                let examples = ctx.graph_repo
                    .and_then(|r| r.example_blocks_for_symbol(&symbol.id).ok())
                    .unwrap_or_default();
                Ok(build_example_object(symbol, &examples))
            }
            _ => Err(ExplorerError::ViewNotAvailable { ... }),
        }
    }
}

// Pure shaper
pub fn build_example_object(symbol: &ResolvedSymbol, examples: &[ExampleBlock]) -> ContextualView {
    let blocks: Vec<ViewBlock> = if examples.is_empty() {
        vec![ViewBlock { id: "empty".into(), title: "No examples".into(),
            body: json!({ "block_type": "placeholder", "message": "No usage examples found" }) }]
    } else {
        examples.iter().map(|e| ViewBlock {
            id: e.example_id.clone(),
            title: format!("{} @ {}:{}", e.file, e.line, e.language),
            body: json!({
                "block_type": "example",
                "code": e.code,
                "language": e.language,
                "file": e.file,
                "line": e.line,
                "description": e.description,
            }),
        }).collect()
    };
    ContextualView { object_id: symbol.id.clone(), view_id: "example-object".into(),
        title: format!("Examples: {}", symbol.name), view_kind: ViewKind::ExampleObject,
        renderer_kind: RendererKind::Composite, blocks, relations: vec![],
        evidence: vec![], findings: vec![] }
}
```

## Graph Repository Extension

The `GraphRepository` port needs a new method:

```rust
// in graph_repository.rs
fn example_blocks_for_symbol(&self, symbol_id: &str) -> GraphResult<Vec<ExampleBlock>>;
```

This queries graph nodes of kind `Doc` with a `example_of` relation pointing to the symbol.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `build_example_object` with 0/1/many examples | Direct shaper call with `ExampleBlock` fixtures |
| Unit | `ExampleObjectExecutor::build` dispatches to `Symbol` and returns error for non-Symbol | Mock `ViewContext` |
| Integration | Registry lists `example-object` for `InspectableObjectType::Symbol` | `list_for(Symbol)` assertion |

## Open Questions

- [ ] Does `example_blocks_for_symbol` exist on `GraphRepository`? If not, what query pattern should it use?
- [ ] What is the canonical graph node kind for examples — `Doc` with a marker property, or a dedicated `Example` node kind?
- [ ] Should examples include a `description` field, or is code + location sufficient?
