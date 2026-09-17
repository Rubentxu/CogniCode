# CP0 — Backstage Fit Spike

**Disposable.** This subtree exists only to answer one question: *is Backstage an
adequate **optional host shell** for the CogniCode outer-loop Control Plane,
while CogniCode stays headless/authoritative and the Explorer stays the
specialized inner-loop product?*

See `openspec/changes/cp0-backstage-fit-spike/` for the full record.

## Shape

```text
Backstage frontend plugin  (packages/plugin-cognicode)
        │  /api/cognicode/probe
        ▼
Backstage backend plugin   (packages/plugin-cognicode-backend)
        │  HTTP GET /control-plane/probe
        ▼
CogniCode Rust service (Explorer API)
```

No CogniCode domain logic lives in TypeScript. The backend plugin holds no
canonical state.

## Run

```bash
cd integrations/backstage
npm install --legacy-peer-deps   # Backstage is yarn-centric; npm needs this flag
npx vitest run                   # 17 spike tests
```

`--legacy-peer-deps` is required because Backstage pins React 18 types while a
default npm resolution pulls React 19 (`@types/react@19`). Backstage's own
tooling assumes Yarn, which is not installed here.

## Removability

Deleting `integrations/backstage/` removes the spike completely. No CogniCode
crate, no workspace member, no root config depends on it.
