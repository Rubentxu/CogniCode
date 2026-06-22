import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

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
   * Tailwind 4 reads `src/tailwind.css` directly via the Vite plugin.
   * No PostCSS config required (CSS-first @theme block).
   */
  envPrefix: "VITE_",
});
