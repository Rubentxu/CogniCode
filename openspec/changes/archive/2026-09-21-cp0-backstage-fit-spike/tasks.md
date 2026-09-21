# CP0 — Tasks

> Operational authority is `state.yaml`. This file is the human-readable plan.

| WU | Task | State |
|---|---|---|
| WU1 | Toolchain characterization (node/npm/yarn) | DONE |
| WU2 | Package resolution under npm (`--legacy-peer-deps`) | DONE |
| WU3 | Rust `GET /control-plane/probe` (read-only, stateless, non-authoritative) | DONE |
| WU4 | Backend plugin `createBackendPlugin` + `/probe` proxy | DONE |
| WU5 | Frontend plugin `createFrontendPlugin` + `/cognicode` page | SOURCE ONLY (unbuilt) |
| WU6 | ContextCapsule v1 + Explorer deep link | DONE |
| WU7 | Catalog Component annotation mapping (`cognicode.io/workspace-id`) | NOT DONE |
| WU8 | Actor→identity context propagation | DONE |
| WU9 | Removability (bounded subtree, no workspace coupling) | DONE |
| WU10 | No canonical state in the host; restart/replica safe | DONE |
| WU11 | Unavailable reported as unavailable (never stale truth) | DONE |
| WU12 | UAT CP0-U1…U12 | PARTIAL (see verification-report) |
| WU16 | Fit decision matrix + single outcome | DONE |
