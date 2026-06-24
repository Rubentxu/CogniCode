import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// WASM plugin — required for cognicode_graph_wasm module.
// Add to package.json devDependencies: "vite-plugin-wasm": "^3.3.0"
// Then run: wasm-pack build crates/cognicode-graph-wasm \
//             --target web --out-dir ../../apps/explorer-ui/src/wasm
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import wasm from "vite-plugin-wasm";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // Handles .wasm imports — Vite serves them as URLs.
    // NOTE: If you see a build error about missing vite-plugin-wasm,
    // run: npm install -D vite-plugin-wasm
    wasm(),
  ],

  /**
   * Dev server proxies all `/api/*` traffic to the `explorer-api` axum
   * binary (see `crates/cognicode-runtime/src/bin/api.rs`). The default
   * listen port for that binary is `127.0.0.1:8010`; override with the
   * `EXPLORER_API_TARGET` env var when running the backend on a
   * different host/port (e.g. behind docker-compose or in CI).
   *
   * See: crates/cognicode-explorer (Rust) — 11 REST endpoints under /api/*.
   */
  server: {
    port: 5173,
    strictPort: false,
    proxy: {
      "/api": {
        target:
          process.env.EXPLORER_API_TARGET ?? "http://127.0.0.1:8010",
        changeOrigin: true,
        secure: false,
      },
    },
  },

  /**
   * Test isolation: keep tests out of the dev/build pipeline.
   */
  build: {
    outDir: "dist",
    sourcemap: true,
    target: "es2022",
  },

  /**
   * WASM module: don't pre-bundle or optimize this dependency.
   * The wasm-pack output must be served as-is; pre-bundling breaks
   * the module's internal WASM loading logic.
   */
  optimizeDeps: {
    exclude: ["cognicode_graph_wasm"],
  },

  /**
   * Tailwind 4 reads `src/tailwind.css` directly via the Vite plugin.
   * No PostCSS config required (CSS-first @theme block).
   */
  envPrefix: "VITE_",
});
