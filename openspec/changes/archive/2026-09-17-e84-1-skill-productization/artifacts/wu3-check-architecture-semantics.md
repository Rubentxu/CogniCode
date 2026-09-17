# WU3 — `check_architecture` semantics (NOT e77)

## What `check_architecture` actually does today

The runtime exposes `check_architecture` as a stable MCP tool. Its
implementation is in
`crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs:1399-1427`:

```rust
fn check_architecture_internal(ctx: &HandlerContext)
    -> HandlerResult<Option<ArchitectureResult>>
{
    let graph = ctx.analysis_service.get_project_graph();
    if graph.symbol_count() == 0 { return Ok(None); }

    let cycle_detector = CycleDetector::new();
    let cycle_result = cycle_detector.detect_cycles(&graph);
    // ... builds CycleInfo list + score = 100 - 5 * symbols_in_cycles
    Ok(Some(ArchitectureResult { cycles, violations: vec![], score, ... }))
}
```

It returns:

- `cycles: Vec<CycleInfo>` — strongly-connected components in the
  **call graph** (functions that recursively reach each other).
- `score: f32` — `100 - 5 * symbols_in_cycles`, clamped to 0.
- `summary: String` — `"N cycles detected"`.

It does **not** return:

- Architectural rules / governance policies.
- Layer / port / adapter violations.
- ADR-derived expected architecture comparisons.
- Executable architecture rules (e77).

## What `check_architecture` is NOT

- **It is NOT the e77 executable architecture product surface.**
  e77 introduced "Architecture knowledge as versioned executable
  constraints" (ADR-049). The runtime MCP today has no tool that
  exposes that subsystem. There is no e77 adapter yet.
- **It is NOT a layer-violation detector** (no port/adapter
  boundaries, no "controller should not import from persistence",
  no import-policy enforcement).
- **It is NOT an ADR-conformance checker.**
- **It is NOT a "diff against expected architecture" tool.**

## What it IS

A graph cycle detector. Useful for finding:

- Mutual recursion (a ↔ b).
- Circular call dependencies (a → b → c → a).
- Architectural smells at the call-graph level (large cycles,
  hubs that participate in many cycles).

## Skill wording (mandatory)

A skill that mentions `check_architecture` MUST say so explicitly:

```text
check_architecture is a Tarjan SCC cycle detector over the
project call graph. It detects mutual recursion and circular
call dependencies. It is NOT the e77 executable-architecture
subsystem; there is no public MCP adapter for that yet.
```

If the skill claims e77 capabilities through `check_architecture`,
it is product-fiction. Reject.

## Future

When an e77 adapter is built, it should be a **separate** MCP tool
with a different name (suggested: `check_executable_architecture` or
`evaluate_architecture_rules`). Skills should be taught to prefer
the new tool once it exists; until then, do not promise e77
behaviour through `check_architecture`.

## Out of scope for e84.1

Building the e77 adapter. Recorded in WU19 as a future product
surface.
