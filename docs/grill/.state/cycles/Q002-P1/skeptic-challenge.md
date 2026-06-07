# Q002-P1 Skeptic Challenge

**Key Challenge**: Leptos dashboard already exists (18 pages, CodeExplorerPage 446 lines). Proxy answer ignores it.

**Valid concerns** (non-Leptos-related):
1. pnpm vs npm: root uses npm (package-lock.json). Switch needs justification.
2. Three-directory premature layering: `apps/` + `packages/types` + `packages/wasm` for MVP.
3. ts-rs types staleness: CI-generate vs commit?
4. Vite 6 vs Vite 7: Vite 7 stable by June 2026.

**User already clarified in Q001-P1**: "no vamos a usar leptos. el dashboard es otra aplicacion independiente, no la relaciones."
