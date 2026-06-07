# Q004-P1 Judge Decision

**Verdict**: MODIFIED

**Final answer**: 
- **SWR** (~4KB) for server state — caching, dedup, refetch for 7 API endpoints. Lighter than TanStack Query for this scale.
- **React Context + useReducer** for Miller Columns state — the ONLY shared UI state (columns array, active column index).
- Per-column state: **local useState** (active view, active lens, playground query).
- Spotter: **cmdk-internal** (already handled).
- **No typed fetch wrapper** — SWR's `fetcher` parameter IS the typed wrapper. Delete ~50 lines.
- **No Zustand, no Jotai** — Context + useReducer sufficient for a single shared state slice.

**Why**: Follows Q003 pattern — minimize dependencies. SWR covers all needed server state patterns at 4KB. The Miller Columns array is the only shared UI state — Context is the right tool, not a global store. Per-column state stays local to each column component. Zero unnecessary abstractions.
