# ViewSpec JSONata Sandbox Specification

## Purpose

Define the client-side, sandboxed JSONata execution service that powers live preview in the `ViewSpecWizard`. The sandbox is a dedicated Web Worker that loads the `jsonata` npm package lazily, enforces a per-evaluation CPU budget, caps the input size, and reports structured errors without blocking the main thread.

## Requirements

### Requirement: 1. Web Worker execution

The system MUST execute JSONata expressions in a dedicated Web Worker at `apps/explorer-ui/src/workers/jsonata.worker.ts`, the only module that imports the `jsonata` npm package.

| Field | Type | Notes |
|-------|------|-------|
| `expression` | `string` | The JSONata source to evaluate |
| `input` | `unknown` | The data (MoldQL result) the expression runs over |

The worker MUST respond with `{ ok: true, output: unknown, duration_ms: number }` on success or `{ ok: false, error: string, duration_ms: number }` on failure.

#### Scenario: Worker evaluates a simple filter

- GIVEN `{ expression: "items[price > 10]", input: { items: [{price: 5}, {price: 20}] } }`
- WHEN the worker runs
- THEN the response is `{ ok: true, output: { items: [{price: 20}] }, duration_ms: <number> }`

#### Scenario: Worker reports a parse error

- GIVEN `{ expression: "items[", input: {...} }`
- WHEN the worker runs
- THEN the response is `{ ok: false, error: <parser message>, duration_ms: <number> }`
- AND the main thread receives the error verbatim

### Requirement: 2. 100 ms evaluation budget

The host MUST call `worker.terminate()` exactly 100 ms after the request is posted, regardless of whether a response has arrived, then resolve with `{ ok: false, error: "budget_exceeded", duration_ms: 100 }`. A new evaluation MUST spawn a fresh worker; the terminated worker is never reused.

#### Scenario: Looping expression is killed

- GIVEN a JSONata expression that loops for 10 s
- WHEN the worker is invoked
- THEN at 100 ms the host calls `worker.terminate()`
- AND the wizard displays "Transform exceeded 100 ms budget — simplify the expression"

#### Scenario: Normal expression completes within budget

- GIVEN an expression that evaluates in 5 ms
- WHEN the worker is invoked
- THEN the response is delivered in < 100 ms with `ok: true` and the actual `duration_ms`

### Requirement: 3. 1 MB input size cap

Before posting, the host MUST serialise `input` to UTF-8 and reject any payload whose byte length exceeds 1 048 576 bytes. The check MUST happen on the main thread. A rejected payload MUST surface as `{ ok: false, error: "input_exceeds_1mb_cap" }` without spawning a worker.

#### Scenario: 5 MB input is rejected

- GIVEN an `input` of 5 MB
- WHEN the host checks the byte length
- THEN the worker is never spawned
- AND the wizard displays "Input exceeds 1 MB cap — narrow the MoldQL query"

#### Scenario: 500 kB input is accepted

- GIVEN an `input` of 500 kB
- WHEN the host checks the byte length
- THEN the worker is spawned and the expression runs normally

### Requirement: 4. Lazy-load on demand

The `jsonata` worker module MUST be imported via dynamic `import()` so the ~150 KB gzipped payload is excluded from the main bundle. The lazy import MUST be triggered the first time the user opens `TransformStep`; subsequent invocations reuse the loaded module.

#### Scenario: Initial bundle has no jsonata code

- GIVEN the explorer-ui production build before any wizard opens
- WHEN `vite build` runs and the bundle is inspected
- THEN the main chunk does NOT contain the string `jsonata.evaluate`

#### Scenario: First wizard open loads the worker

- GIVEN the explorer is idle
- WHEN the user reaches step 4 of the wizard for the first time
- THEN a network request for `jsonata.worker.ts` fires
- AND the worker is spawned

### Requirement: 5. Race cancellation

When a new evaluation starts while a previous one is in flight, the host MUST call `worker.terminate()` on the in-flight worker and ignore its eventual response. Only the most recent request's response updates the wizard state.

#### Scenario: Fast typing cancels stale evaluations

- GIVEN the user types `"items"`, then `"items["`, then `"items[*]"` within 1 s
- WHEN each keystroke debounces and triggers the worker
- THEN at most one response is reflected in the UI
- AND the two earlier workers are terminated before their results land

## Out of Scope

- JSONata execution on the Rust backend (backend just stores the expression)
- Persistent worker pool / worker reuse across evaluations
- Per-execution memory cap (only CPU time and input size are bounded)

## Coverage

- **Happy paths**: covered (simple eval, parse error, normal completion)
- **Edge cases**: covered (looping expression, oversized input, lazy load, race)
- **Error states**: covered (budget exceeded, parse error, input cap)
