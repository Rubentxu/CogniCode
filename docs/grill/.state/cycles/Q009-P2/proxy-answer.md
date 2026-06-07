# Q009-P2 Proxy Answer: Visualization

**Answer**: Custom React SVG components, fed by JSON layout data computed in WASM from cognicode-diagram's Sugiyama engine. No graph rendering library. Bundle: ~0KB for SVG vs ~200KB for React Flow.

**Why**: cognicode-diagram crate already has layout engine (756 lines) + SVG renderer. Compile to WASM. Frontend renders SVG primitives in React JSX with click handlers for Miller Column navigation. Navigation, not graph manipulation.
