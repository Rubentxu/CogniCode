/**
 * The UI must not request routes the Explorer server does not serve.
 *
 * Why this exists: the E2E suite runs with `VITE_USE_MOCKS=true`, and the MSW
 * handlers mirror the UI instead of the server, so a UI that asks for a route
 * the axum router lacks stays green until it meets the real backend.
 *
 * This is a cheap static check: it reads the route table from `api.rs` and the
 * URL templates the hooks build, and fails naming each URL with no route.
 * It starts no browser and no server.
 *
 * The check is a ratchet. `KNOWN_UNSERVED` lists URLs that were already
 * unserved when the check was introduced and that belong to other work; a
 * new unserved URL fails, and so does a listed URL that gains a route (the
 * entry must then be deleted, so the list can only shrink).
 */
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const apiRs = path.resolve(
  here,
  "../../../../crates/cognicode-explorer/src/api.rs",
);
const hooksDir = path.resolve(here, "../hooks");

/**
 * No `.route(...)` in `crates/` serves any of these.
 * `ScanBar` needs `/scan`, `/jobs/:p` and `/graph/stats` together, so serving
 * one of them alone would not make it work; whether to implement the three or
 * retire the component is a separate decision.
 */
const KNOWN_UNSERVED = new Set([
  "/api/ask",
  "/api/jobs/:p",
  "/api/workspaces/:p",
  "/api/workspaces/:p/scan",
  "/api/workspaces/:p/graph/stats",
  "/api/workspaces/:p/index",
  "/api/exploration-sessions/:p/artifacts/:p",
]);

/** `:object_id` and `${expr}` both become `:p`, so shapes compare. */
function normalize(url: string): string {
  return url
    .replace(/\?.*$/, "")
    .replace(/\/\$\{[^}]*\}/g, "/:p")
    .replace(/\$\{[^}]*\}/g, "")
    .replace(/:[A-Za-z_][A-Za-z0-9_]*/g, ":p");
}

function serverRoutes(): Set<string> {
  const source = readFileSync(apiRs, "utf8");
  const routes = new Set<string>();
  for (const m of source.matchAll(/\.route\(\s*"([^"]+)"/g)) {
    routes.add(normalize(m[1] ?? ""));
  }
  return routes;
}

/** URL templates in the hooks: backtick literals that start with `/word/`. */
function uiRequests(): Map<string, string> {
  const found = new Map<string, string>();
  for (const file of readdirSync(hooksDir)) {
    if (!/\.tsx?$/.test(file) || /\.(test|spec)\./.test(file)) continue;
    const source = readFileSync(path.join(hooksDir, file), "utf8");
    for (const m of source.matchAll(/`(\/[a-z][a-z-]*\/[^`]*)`/g)) {
      const url = normalize(m[1] ?? "");
      // Some hooks spell the `/api` prefix, others leave it to the client.
      found.set(url.startsWith("/api/") ? url : `/api${url}`, file);
    }
  }
  return found;
}

describe("UI requests against the Explorer router", () => {
  it("finds the server route table and some UI requests (guard against a vacuous pass)", () => {
    expect(serverRoutes().size).toBeGreaterThan(10);
    expect(uiRequests().size).toBeGreaterThan(5);
  });

  it("every URL the hooks build has a route in api.rs", () => {
    const routes = serverRoutes();
    const missing = [...uiRequests()]
      .filter(([url]) => !routes.has(url) && !KNOWN_UNSERVED.has(url))
      .map(([url, file]) => `${url}  (${file})`);
    expect(
      missing,
      `routes the UI asks for but the server lacks:\n${missing.join("\n")}`,
    ).toEqual([]);
  });

  it("KNOWN_UNSERVED holds only URLs that are still requested and still unserved", () => {
    const routes = serverRoutes();
    const requested = uiRequests();
    const stale = [...KNOWN_UNSERVED].filter(
      (url) => routes.has(url) || !requested.has(url),
    );
    expect(stale, `delete these from KNOWN_UNSERVED:\n${stale.join("\n")}`).toEqual([]);
  });
});
