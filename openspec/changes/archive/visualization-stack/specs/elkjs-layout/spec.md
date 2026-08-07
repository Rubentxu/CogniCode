# Spec: elkjs-layout

> New capability. Companion to proposal `sdd/visualization-stack/proposal`.
> Layout engine for `interactive-graph`. Runs in a Web Worker so that
> Cytoscape.js rendering stays on the main thread and the UI remains
> responsive on 200+ node graphs.

## Purpose

A Web Worker that wraps `elkjs` (Eclipse Layout Kernel for JavaScript) and
exposes three layout algorithms — `layered` (default), `force`, and `radial` —
over a `comlink` RPC surface. The worker is the single source of truth for
graph positioning; `InteractiveGraph` calls into it and never computes
positions inline. Streaming progress (partial layouts) is supported so the
renderer can paint incrementally on large graphs.

## Requirements

### Requirement 1: Worker module surface

The worker MUST live at
`apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts` and
MUST be instantiated via Vite's `?worker` import:

```ts
import LayoutWorker from './layout.worker.ts?worker';
const worker = new LayoutWorker();
```

The worker MUST expose, via `comlink.expose`, a single object with the
following shape:

| Method                         | Returns                  | Purpose                            |
|--------------------------------|--------------------------|------------------------------------|
| `layout(elements, options)`    | `Promise<LayoutResult>`  | Compute positions for all elements |
| `cancel()`                     | `void`                   | Abort the in-flight computation    |
| `onProgress(cb)`               | `() => void` (unsubscribe) | Subscribe to progress (0..1)     |

The main thread MUST consume the worker via `comlink.wrap`, not by
post-message protocol directly.

#### Scenario: Worker exposes the three documented methods

- GIVEN the worker module
- WHEN `comlink.expose` is called with the API object
- THEN the object has the keys `"layout"`, `"cancel"`, `"onProgress"` and
  no others (verified by `Object.keys(api).sort()` in a unit test of the
  worker source)

#### Scenario: Main thread uses comlink.wrap, not direct postMessage

- GIVEN `layout.worker.ts` consumer
- WHEN the import is inspected
- THEN `comlink.wrap(new LayoutWorker())` is used AND no raw
  `worker.postMessage(...)` / `worker.addEventListener('message', ...)`
  pair exists in the consumer

### Requirement 2: Layout algorithms and options

`layout(elements, options)` MUST accept:

| Option                | Type                                | Default       | Meaning                                |
|-----------------------|-------------------------------------|---------------|----------------------------------------|
| `algorithm`           | `"layered" \| "force" \| "radial"`  | `"layered"`   | Layout algorithm to apply              |
| `width`               | `number`                            | `1024`        | Target canvas width (px)               |
| `height`              | `number`                            | `768`         | Target canvas height (px)              |
| `nodeSeparation`      | `number`                            | `80`          | Min horizontal distance between nodes  |
| `rankSeparation`      | `number`                            | `100`         | Min vertical distance between layers   |
| `direction`           | `"LR" \| "TB" \| "RL" \| "BT"`     | `"LR"`        | Layered algorithm direction            |
| `iterations`          | `number`                            | `300`         | Force-algorithm iteration cap          |
| `animate`             | `boolean`                           | `false`       | Stream intermediate positions          |

An unknown `algorithm` MUST be rejected with an error of shape
`{name:"InvalidLayoutOption", message:"unknown algorithm: <value>"}` —
the promise MUST reject (not resolve with garbage).

#### Scenario: Default options produce a layered LR layout

- GIVEN `layout(elements, {})` with no options
- WHEN the promise resolves
- THEN every node has a `position: {x: number, y: number}` AND the
  algorithm used (asserted via a test spy on `elk.layout`) is `layered`
  AND the direction is `LR`

#### Scenario: `algorithm: "force"` runs the force-based layout

- GIVEN `layout(elements, {algorithm: "force", iterations: 50})`
- WHEN the promise resolves
- THEN the test spy on `elk.layout` is called with an elk options object
  whose `algorithm` is `"force"` (or the elkjs `force` algorithm
  identifier) AND `iterations` is 50

#### Scenario: `algorithm: "radial"` runs the radial layout

- GIVEN `layout(elements, {algorithm: "radial"})`
- WHEN the promise resolves
- THEN the test spy on `elk.layout` is called with `algorithm: "radial"`

#### Scenario: Unknown algorithm rejects

- GIVEN `layout(elements, {algorithm: "bogus"})`
- WHEN the promise settles
- THEN it rejects with an error whose `name` is `"InvalidLayoutOption"`
  AND whose `message` includes the string `"bogus"`

### Requirement 3: Streaming progress

When `options.animate === true`, the worker MUST emit progress callbacks
with monotonically non-decreasing values in `[0, 1]`, terminating with a
final callback at exactly `1.0` immediately before the promise resolves.
Subscribers registered via `onProgress(cb)` MUST receive every value
emitted between the time of subscription and the final resolution.
Multiple subscribers MUST each receive every value.

When `options.animate === false` (default), the worker MUST emit exactly
one progress event with value `1.0` immediately before the promise
resolves, and no intermediate values.

#### Scenario: Animate=true emits at least 3 progress events

- GIVEN a 200-node graph and `{algorithm: "layered", animate: true}`
- WHEN the layout is computed
- THEN the subscriber receives ≥3 progress callbacks AND the last
  callback value is `1.0` AND no callback value is `<` the previous
  value (monotonic)

#### Scenario: Animate=false emits exactly one progress event

- GIVEN a 50-node graph and `{animate: false}` (default)
- WHEN the layout is computed
- THEN the subscriber receives exactly 1 progress callback with value
  `1.0` AND the promise resolves immediately after

#### Scenario: Two subscribers both receive every value

- GIVEN two `onProgress` subscribers registered before the layout starts
- WHEN the layout streams progress
- THEN both subscribers observe the same sequence of values (asserted by
  pushing values into two arrays and comparing)

### Requirement 4: Cancellation

`cancel()` MUST abort the in-flight layout computation. After `cancel()`
returns, the promise returned by the in-flight `layout(...)` call MUST
reject with an error whose `name` is `"LayoutCancelled"`. Subsequent
calls to `layout(...)` after a `cancel()` MUST succeed normally (no
"poisoned worker" state). `cancel()` with no layout in flight MUST be a
no-op (no throw).

#### Scenario: Cancel rejects the in-flight promise

- GIVEN an in-flight `layout(largeGraph, {animate: true})`
- WHEN `cancel()` is called
- THEN the layout promise rejects with `error.name === "LayoutCancelled"`
  AND no further progress callbacks are delivered

#### Scenario: Cancel between layouts is a no-op

- GIVEN a worker that has just resolved a layout
- WHEN `cancel()` is called
- THEN no exception is thrown AND the next `layout(...)` resolves normally

#### Scenario: Worker recovers after a cancel

- GIVEN the previous scenario
- WHEN a new `layout(elements, {algorithm: "layered"})` is started
- THEN it resolves within the documented time budget AND emits a final
  `1.0` progress event

### Requirement 5: Performance budget

The layout step for a 200-node graph with the default `layered` algorithm
MUST complete in under `500ms` on a modern laptop (test environment:
Playwright Chromium, headless, no throttling). For graphs >500 nodes,
`animate: true` MUST be required — the worker MUST reject (not hang)
`animate: false` for inputs whose node count exceeds 500, with
`{name: "LayoutTooLarge", message: "use animate: true for >500 nodes"}`.

#### Scenario: 200-node layered layout completes under 500ms

- GIVEN a synthetic 200-node layered graph
- WHEN `layout(graph, {algorithm: "layered"})` is timed
- THEN `performance.now()` delta is `< 500` ms (Playwright trace)

#### Scenario: 600-node graph with animate=false is rejected

- GIVEN a synthetic 600-node graph
- WHEN `layout(graph, {animate: false})` is called
- THEN the promise rejects with `error.name === "LayoutTooLarge"` AND
  the message contains `"animate: true"` AND the elapsed time is
  `< 100ms` (no hang)

#### Scenario: 600-node graph with animate=true eventually resolves

- GIVEN the previous scenario
- WHEN `layout(graph, {animate: true})` is called
- THEN the promise eventually resolves AND every node has a position
  AND at least 3 progress events were emitted

## Acceptance Criteria

| #   | Criterion                                                                | Verifies |
| --- | ------------------------------------------------------------------------ | -------- |
| AC1 | Worker is a real `?worker` module and uses `comlink.expose` / `wrap`     | R1       |
| AC2 | `layout`, `cancel`, `onProgress` are the only public methods             | R1       |
| AC3 | Defaults match: `layered`, `LR`, `1024x768`, `nodeSeparation=80`         | R2       |
| AC4 | All three algorithms actually delegate to elkjs (verified via spy)       | R2       |
| AC5 | Unknown algorithm rejects with the documented error name                 | R2       |
| AC6 | Streaming is monotonic; terminal value is exactly 1.0                    | R3       |
| AC7 | `cancel()` rejects in-flight; worker recovers afterwards                 | R4       |
| AC8 | 200-node layered layout < 500ms; >500 nodes requires `animate: true`     | R5       |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                                      | Expected behavior                                  |
| --- | ----------------------------------------- | -------------------------------------------------- |
| E1  | `elements` is empty `[]`                  | Resolves with `{positions: []}`, zero progress events after the terminal `1.0` |
| E2  | Single node, no edges                     | Resolves with one position, no errors              |
| E3  | Self-loop edge                            | Layout still completes; node positions valid       |
| E4  | Cycle in graph                            | Layout still completes (elk handles cycles)        |
| E5  | Disconnected components                   | Each component is laid out independently; all nodes have positions |
| E6  | Negative `width` / `height`               | Rejects with `InvalidLayoutOption`                 |
| E7  | `iterations: 0` (force)                   | Rejects with `InvalidLayoutOption`                 |
| E8  | Worker is terminated mid-layout           | Promise rejects with `LayoutCancelled` (or `WorkerTerminated`) |
| E9  | Subscriber added after a progress event   | Does NOT receive past events (only future ones)    |
| E10 | `cancel()` called twice in a row          | No throw, no double-reject                         |
| E11 | `layout()` called twice concurrently      | Both promises resolve independently with correct positions (no shared mutable state) |
| E12 | `animate: true` on a 50-node graph        | Still works; progress may skip rapidly — last value is 1.0 |
| E13 | `algorithm: "force"` on a 600-node graph with `animate: false` | Rejects with `LayoutTooLarge`             |
| E14 | Node with extremely long label (1000+ chars) | Layout does not throw; bounding box may exceed canvas but position is finite |

## TDD RED Gate

Before implementation is considered started, the following tests MUST exist
and FAIL (RED).

| Test file                                                       | Requirement | Status |
|-----------------------------------------------------------------|-------------|--------|
| `layout.worker.test.ts::worker_exposes_three_methods`           | R1          | RED    |
| `...::main_thread_uses_comlink_wrap`                            | R1          | RED    |
| `...::default_options_layered_lr`                               | R2          | RED    |
| `...::force_algorithm_runs`                                     | R2          | RED    |
| `...::radial_algorithm_runs`                                    | R2          | RED    |
| `...::unknown_algorithm_rejects`                                | R2          | RED    |
| `...::animate_true_emits_monotonic_progress`                    | R3          | RED    |
| `...::animate_false_emits_exactly_one_event`                    | R3          | RED    |
| `...::two_subscribers_both_receive`                             | R3          | RED    |
| `...::cancel_rejects_in_flight`                                 | R4          | RED    |
| `...::cancel_between_layouts_is_noop`                           | R4          | RED    |
| `...::worker_recovers_after_cancel`                             | R4          | RED    |
| `layout.bench.test.ts::200_node_layered_under_500ms`            | R5          | RED    |
| `...::600_node_animate_false_rejected`                          | R5          | RED    |
| `...::600_node_animate_true_resolves`                           | R5          | RED    |

## Out of Scope (locked)

- D3.js force layouts — only elkjs is used
- Server-side layout computation
- Persisting computed layouts to IndexedDB / localStorage
- Animated transitions between two layouts (only streaming within a single
  layout is in scope)
- Layouts for non-Cytoscape renderers
- Custom user-supplied layout algorithms (the three documented ones are
  exhaustive for this change)
- Cluster / compound node layout (Cytoscape `compound` semantics; deferred
  to a later change)
