# Q007-P2 Proxy Answer: Loading States

**Answer**: React 19 Suspense at top level for WASM init. SWR's isLoading for data. Three tiers:
1. WASM init: full-page branded spinner (critical path)
2. First data fetch: lightweight column outline placeholder with pulse animation (Tailwind `animate-pulse`)
3. Cache hit: instant (SWR stale-while-revalidate)

No skeleton library. No per-component skeletons. Suspense boundary at app root.

# Q008-P2 Proxy Answer: Error States

**Answer**: Error Boundary per Miller Column. SWR error state for API errors with retry. App-level boundary as last resort.
- Column crash: "Something went wrong" message + "Try again" button
- API error: SWR's error + retry, fallback to last cached data
- App crash: app-level Error Boundary with full-page fallback
- React 19's built-in ErrorBoundary (no library)
