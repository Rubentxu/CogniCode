# Q002-P1 Proxy Answer

**Question**: Where does the frontend live and what build tool?

**Answer**: `apps/explorer-ui/` using Vite 6 + React 19 plugin + TypeScript strict + Tailwind CSS v4. Root package.json gains `"workspaces": ["apps/*", "packages/*"]`. WASM layer in separate `packages/cognicode-explorer-wasm/` built via wasm-pack. Dev proxy to Explorer backend via Vite `server.proxy`.

**Confidence**: high
**Needs user validation**: true (backend port, auto-generation commit strategy)
