# Q004-P1 Skeptic Challenge

**Key concerns**:
1. SWR (~4KB) lighter than TanStack Query (~12KB) for 7 endpoints — same Q003 minimize_dependencies pattern
2. Zustand columns[] array re-renders ALL columns on any mutation. Jotai atoms isolate per-column.
3. Typed fetch wrapper ~50 lines is redundant — SWR's `fetcher` parameter IS the typed wrapper
4. Miller Columns UI state may not need Zustand — React Context + useReducer inside component tree
5. WASM integration path undefined

**Suggested**: SWR + Jotai (atoms per column). Delete typed fetch wrapper.
