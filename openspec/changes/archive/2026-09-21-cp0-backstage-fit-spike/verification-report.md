# CP0 — Verification Report

## Executed evidence (real, reproducible)

| Evidence | Command | Result |
|---|---|---|
| Spike unit tests | `cd integrations/backstage && npx vitest run` | **3 files, 17 tests PASSED** |
| ContextCapsule v1 | `src/context-capsule.test.ts` | 7 passed |
| Probe client | `src/cognicode-probe-client.test.ts` | 6 passed |
| Backend plugin | `packages/plugin-cognicode-backend/src/index.test.ts` | 4 passed |
| Rust route present | `grep -n control-plane/probe crates/cognicode-explorer/src/api.rs` | registered on both `router` and `router_with_state` |
| Rust compiles (shipping config) | `cargo check -p cognicode-explorer` (default features) | clean |

### What the 17 tests actually prove

- **Bounded, versioned, references-only capsule.** Unknown versions are rejected;
  focus is deduped+sorted (deterministic); over-cap and over-length inputs are
  rejected rather than repaired.
- **No authority in the handoff.** The capsule's key set is asserted exactly and
  asserted to contain no approval/authorization/permit/promotion/actor/evidence
  field. The probe response reports `authority: none`.
- **Service-backed, no canonical state.** Two successive probe calls are
  independent (changed upstream version is observed on the second call, proving
  no caching), which is what makes the host restart/replica safe.
- **Unavailable is unavailable.** Connection failure, HTTP failure, malformed
  JSON, and a contract-violating body all raise `CogniCodeUnavailableError`;
  the router maps that to **503 `{status:"unavailable"}`**. It never serves stale
  or fabricated data as current truth.
- **Identity is context.** The host actor is forwarded explicitly as
  `x-cognicode-actor-subject` + `-source`, and omitted when the host has no
  authenticated actor.

## NOT executed — and why (honesty over completeness)

| Not executed | Reason |
|---|---|
| Frontend plugin build/typecheck | `@backstage/frontend-plugin-api` is ESM-only with React peer conditions Node could not resolve here; React 18 pins vs npm's React 19. The source is authored against the documented modern frontend-system API but **is not compiled or typechecked**. |
| UAT CP0-U1/U2 (page renders, navigation) | Requires the above frontend build. |
| UAT CP0-U4/U5 (interactive handoff in a browser) | Same. |
| Real end-to-end browser handoff | Same. |
| Catalog Component annotation mapping (WU7) | Deferred; not needed to answer the fit question, since the answer turns on the *shape* of the integration, not catalog depth. |

**Therefore the frontend half of the integration is claimed as designed and
source-authored, not as verified running software.** The backend half and the
handoff contract are verified by real passing tests.

## Pre-existing breakage observed (not caused by CP0)

`cargo check -p cognicode-explorer --features multimodal` fails with **17 errors
in `domain/views.rs`** at pristine HEAD. Default (shipping) features compile
clean, so the probe is fine in the shipped configuration. Recorded, not fixed.

## Impact on the LSI invariants

None. No Rust domain, application, or governance code was modified by this cycle
beyond the additive, read-only probe route. Authority invariants are untouched.
`check_architecture` and e78 are untouched by design.
