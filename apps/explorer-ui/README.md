# CogniCode Explorer — Frontend

React 19 + TypeScript (strict) + Tailwind CSS 4 + Vite 8 single-page app at
`apps/explorer-ui/`. Consumes the 11 axum REST endpoints exposed by
`crates/cognicode-explorer`.

## Status

- **Phase 0**: validation spikes — *deferred to verify phase*
- **Phase 1**: ✅ Scaffold (this commit). Vite + React 19 + TS strict +
  Tailwind 4 + Vitest + Playwright wired. ErrorBoundary and LoadingTier
  primitives in place. Zero business logic yet.
- **Phase 2+**: panel wiring, SWR hooks, Zod schemas, Zustand-free state
  management via `Context + useReducer`.

## Stack

| Concern | Choice |
|---------|--------|
| UI framework | React 19.2 |
| Build tool | Vite 8 |
| Language | TypeScript 6 (strict, `noUncheckedIndexedAccess`) |
| Styling | Tailwind CSS 4 (CSS-first `@theme` block) |
| Data fetching | SWR 2.4 |
| Validation | Zod 4 (ViewBlock discriminated union) |
| Search | cmdk 1.1 (Spotter palette) |
| Unit/integration | Vitest 3 + Testing Library + jsdom |
| API mocking | MSW 2.x |
| E2E | Playwright 1.60 |
| A11y | axe-core (added with E2E specs) |

## Scripts

```bash
npm install          # one-time
npm run dev          # start dev server (http://localhost:5173)
npm run build        # type-check + production build
npm run test         # run unit tests once
npm run test:watch   # run unit tests in watch mode
npm run test:ui      # run unit tests in Vitest UI
npm run test:e2e     # run Playwright E2E specs
```

## Backend wiring

The Vite dev server proxies `/api/*` to `http://127.0.0.1:8080` (the axum
backend). Make sure `cargo run -p cognicode-explorer` is running in another
terminal before hitting the UI.

## Layout (current)

```
src/
├── App.tsx                  # Root component
├── App.test.tsx             # Smoke test
├── main.tsx                 # ReactDOM entry
├── tailwind.css             # @theme tokens (dark-only)
├── vite-env.d.ts
├── components/
│   ├── ErrorBoundary.tsx    # Per-panel isolation
│   ├── ErrorBoundary.test.tsx
│   ├── LoadingTier.tsx      # 3-tier loading state (skeleton / spinner / empty)
│   └── LoadingTier.test.tsx
└── test/
    └── setup.ts             # Vitest + jest-dom
```

## Design context

See `docs/grill/2026-06-07-explorer-frontend-gaps.report.md` and the
auto-grill-loop ledger for the 12 decisions that locked this stack.
