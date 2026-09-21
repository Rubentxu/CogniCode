# CP0 — Fit Decision (WU16)

## Outcome

# `FIT_WITH_CONSTRAINTS`

Backstage is an **adequate optional host shell** for a future CogniCode Control
Plane, **provided** the integration stays in the service-backed shape
demonstrated here: host renders and navigates, CogniCode Rust remains the single
source of truth, and no authority crosses the boundary.

## Decision matrix

| Criterion | Weight | Finding | Verdict |
|---|---|---|---|
| Can Backstage host CogniCode without owning domain logic? | critical | Backend plugin is a stateless HTTP proxy; 4 tests assert no state | PASS |
| Can the host avoid becoming a second source of truth? | critical | No canonical state; restart/replica safe (tested) | PASS |
| Can it hand off to the Explorer without embedding it? | high | ContextCapsule v1 + link; no iframe; bounded/versioned/refs-only (7 tests) | PASS |
| Can identity be propagated without propagating authority? | critical | Actor forwarded as context header; probe reports `authority: none` (tested) | PASS |
| Does unavailability avoid becoming false truth? | high | 503 `unavailable` on every failure mode (tested) | PASS |
| Is the subtree removable? | high | `integrations/backstage/` is not a workspace member; no CogniCode crate depends on it | PASS |
| Does the frontend actually build and run here? | high | **Not verified** — ESM/React peer resolution; no browser UAT | FAIL |
| Is Backstage cheap to adopt versus the Explorer? | medium | Adds a second UI stack + Yarn-centric tooling; npm needs `--legacy-peer-deps` | COST |
| Does anything force Backstage? | medium | No. The Control Plane is headless; any shell could host it | NOT FORCED |

## Why not `BACKSTAGE_FIT`

The frontend half is **unverified running software** in this environment, and the
host imposes real adoption cost (a second UI stack, Yarn-centric tooling, peer
resolution friction). Claiming full fit would overstate the evidence.

## Why not `NOT_FIT`

Nothing in the architecture was violated. Every critical criterion passed with
real tests: no domain logic in TypeScript, no canonical state in the host, no
authority crossing the boundary, no stale-truth failure mode. `NOT_FIT` would
require the shape itself to be wrong, and the shape held.

## The constraint that decides it

> Backstage is fit **as a shell**, and only for as long as the Control Plane
> needs multi-plugin host capabilities the Explorer does not provide — catalog,
> ownership, organizational RBAC, plugin ecosystem.

If the Control Plane never needs those, the Explorer alone is the cheaper and
more coherent answer, and Backstage should not be adopted.

## Forward condition (for whenever a Control Plane is actually built)

When the Control Plane exposes real **e77** architecture through a headless read
model, that becomes the **first genuine product consumer** of the
executable-architecture capability. **Re-run the e78 consumer-count checkpoint
at that moment.** CP0 does not resolve e78, and the `check_architecture` MCP tool
must not be used as evidence either way.
