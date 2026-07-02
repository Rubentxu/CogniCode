import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import yaml from "@rollup/plugin-yaml";
import path from "path";

/**
 * Vitest config — separate from vite.config.ts so the dev server (port 5173)
 * is not started by `vitest run` and tests use jsdom.
 *
 * Coverage thresholds align with the explore-frontend design: SWR hooks,
 * Zod schemas, and the useReducer are the regression surfaces.
 */
export default defineConfig({
  plugins: [react(), tailwindcss(), yaml()],
  resolve: {
    alias: {
      "@scaffold-assets": path.resolve(
        __dirname,
        "../../crates/cognicode-explorer/assets",
      ),
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["node_modules", "dist", ".idea", ".git", ".DS_Store"],
    coverage: {
      reporter: ["text", "json-summary", "html"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.{test,spec}.{ts,tsx}",
        "src/test/**",
        "src/main.tsx",
        "src/vite-env.d.ts",
      ],
    },
  },
});
