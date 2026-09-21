# CP0 — Characterization (before code)

## What already exists (measured, not assumed)

| Surface | Reality |
|---|---|
| Rust service | Explorer API on `:7000`; has `GET /control-plane/probe` (added for CP0) |
| Explorer | Specialized inner-loop product (WASM graph UI) |
| MCP | 43 tools exposed to agents |
| CI | Rust workspace + Playwright |
| Backstage | Nothing before CP0 |

## Toolchain characterization (this environment)

```text
node    v25.9.0
npm     11.12.1
pnpm    11.9.0
yarn    MISSING
```

Backstage is **Yarn-centric**. With npm, Backstage packages resolve only under
`--legacy-peer-deps`; without it npm pulls `@types/react@19` and conflicts with
Backstage's React 18 pins.

## The question the spike must not beg

`check_architecture` (MCP) is **not** a consumer of e77 and must not be used as
evidence that Backstage consumes the executable-architecture capability. It
still uses the legacy `CycleDetector` and hard-codes `violations: vec![]`.
CP0 does not change that and does not claim it as proof.
