# Q001-P1 Skeptic Challenge

**Question**: TypeScript or JavaScript for Explorer frontend?

**Key Challenges**:
1. **ARCHITECTURAL FORK** (HIGH): Leptos dashboard already exists with 18 pages, 17 components, 446-line CodeExplorerPage, 61 Playwright tests — all in Rust WASM
2. **ts-rs fragility** (HIGH): Community crate, 5 contributors, complex Rust enums with serde rename_all would break codegen
3. **Build pipeline doubling** (HIGH): `cargo build → ts-rs codegen → npm install → tsc → vite build` vs current single `cargo build`
4. **Prototype already in Leptos** (HIGH): CodeExplorerPage proves the interaction model works in Rust
5. **TypeScript strict mode unnecessary** (MEDIUM): Rust's type system is already inherently strict — Option<T>, Result<T,E>
6. **MCP boundary** (MEDIUM): TypeScript types benefit only the web UI, not MCP consumers

**Suggested correction**: Extend the existing Leptos dashboard with new explorer pages. Zero new build steps, zero new types, zero technology fork.
