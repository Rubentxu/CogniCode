/**
 * MSW handlers for all 11 backend endpoints.
 *
 * Used by:
 * - `npm run dev` (if `VITE_USE_MOCKS=true`) — to develop without a
 *   running axum backend.
 * - Unit tests — to give SWR hooks a real network to talk to.
 *
 * The handlers return the fixtures from `fixtures.ts` with realistic
 * latency (5–15ms) so loading skeletons actually show up.
 */
import { http, HttpResponse, delay } from "msw";

import {
  contextualViewFixture,
  decisionArtifactFixture,
  explorationSessionFixture,
  inspectableObjectFixture,
  lensDescriptorsFixture,
  lensResultFixture,
  spotterResultsFixture,
  workspaceSummaryFixture,
} from "./fixtures";
import {
  largeSubgraphFixture,
  mediumSubgraphFixture,
  rationaleSubgraphFixture,
  smallSubgraphFixture,
} from "./subgraphFixtures";
import { architectureFixture } from "./architectureFixtures";

const LATENCY_MS = 8;

/**
 * Maps a viewId to its ViewKind and RendererKind.
 * Mirrors the backend stamping in `contextual_view()` (AD-2).
 */
function viewIdToKinds(viewId: string): { viewKind: string; rendererKind: string } {
  switch (viewId) {
    case "call-graph":
      return { viewKind: "call_graph", rendererKind: "graph" };
    case "dependency-graph":
      return { viewKind: "dependency_graph", rendererKind: "graph" };
    case "source":
      return { viewKind: "source_view", rendererKind: "code" };
    case "quality":
      return { viewKind: "quality_hotspots", rendererKind: "json" };
    case "overview":
    default:
      return { viewKind: "vertical_slice", rendererKind: "composite" };
  }
}

// In-memory store for exploration sessions (ADR-040 Wave 3 H4 fix).
// Tests rely on this to validate session save/restore round-trip.
export const explorationSessionStore = new Map<string, Record<string, unknown>>();

export const handlers = [
  // -----------------------------------------------------------------------
  // 1. Health
  // -----------------------------------------------------------------------
  http.get("/api/health", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json({ status: "ok", service: "cognicode-explorer" });
  }),

  // -----------------------------------------------------------------------
  // 2a. List workspaces
  // -----------------------------------------------------------------------
  http.get("/api/workspaces", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json([workspaceSummaryFixture]);
  }),

  // -----------------------------------------------------------------------
  // 2b. Open workspace
  // -----------------------------------------------------------------------
  http.post("/api/workspaces/open", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json(workspaceSummaryFixture);
  }),

  // -----------------------------------------------------------------------
  // 2c. Graph stats
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/graph/stats", async ({ params }) => {
    await delay(LATENCY_MS);
    const workspaceId = params["workspace_id"] as string | undefined;
    return HttpResponse.json({
      workspace_id: workspaceId ?? workspaceSummaryFixture.id,
      symbol_count: 128,
      edge_count: 256,
      last_scan_at: workspaceSummaryFixture.last_scan_at ?? new Date().toISOString(),
    });
  }),

  // -----------------------------------------------------------------------
  // 2d. Workspace quality summary
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/quality-summary", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json({
      summary: {
        scope: "workspace",
        rating: "B",
        total_issues: 3,
        debt_minutes: 60,
        by_severity: { blocker: 0, critical: 1, major: 1, minor: 1, info: 0 },
        last_run: "2026-06-07T09:00:00Z",
      },
      issues: [
        {
          id: 1,
          rule_id: "rust:S100",
          severity: "critical",
          category: "safety",
          file: "src/lib.rs",
          line: 42,
          message: "Critical safety issue",
          status: "open",
          object_id: "issue:1",
        },
      ],
    });
  }),

  // -----------------------------------------------------------------------
  // 3. Index workspace — backend returns 501 today
  // -----------------------------------------------------------------------
  http.post("/api/workspaces/:workspace_id/index", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json(
      { error: "Not implemented: workspace indexing will delegate to CogniCode graph/index builders" },
      { status: 501 },
    );
  }),

  // -----------------------------------------------------------------------
  // 4. Spotter
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/spotter", async ({ request }) => {
    await delay(LATENCY_MS);
    const url = new URL(request.url);
    const q = url.searchParams.get("q") ?? "";
    if (q.length === 0) {
      return HttpResponse.json([]);
    }
    // Return discriminated-union format (SpotterSearchResult) to match the
    // real backend. useSpotter validates the full union and unwraps result.
    // Each family carries the same SpotterResult payload (object + score +
    // match_type); only the `kind` discriminant differs.
    return HttpResponse.json(
      spotterResultsFixture.map((hit, i) => ({
        kind: "symbol",
        result: {
          ...hit,
          match_type: i === 0 ? `query:${q}` : hit.match_type,
        },
      })),
    );
  }),

  // -----------------------------------------------------------------------
  // 5. Inspect object
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id", async ({ params }) => {
    await delay(LATENCY_MS);
    const objectId = params["object_id"] as string | undefined;
    if (!objectId) {
      return HttpResponse.json({ error: "object_id required" }, { status: 400 });
    }
    if (objectId === "missing") {
      return HttpResponse.json(
        { error: `Object not found: ${objectId}` },
        { status: 404 },
      );
    }
    return HttpResponse.json({
      ...inspectableObjectFixture,
      id: objectId,
    });
  }),

  // -----------------------------------------------------------------------
  // 6. Available views
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/views", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json(inspectableObjectFixture.available_views);
  }),

  // -----------------------------------------------------------------------
  // 7. Contextual view
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/views/:view_id", async ({ params }) => {
    await delay(LATENCY_MS);
    const objectId = params["object_id"] as string;
    const viewId = params["view_id"] as string;
    const { viewKind, rendererKind } = viewIdToKinds(viewId);
    return HttpResponse.json({
      ...contextualViewFixture,
      object_id: objectId,
      view_id: viewId,
      view_kind: viewKind,
      renderer_kind: rendererKind,
    });
  }),

  // -----------------------------------------------------------------------
  // 8. Available lenses
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/lenses", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json(lensDescriptorsFixture);
  }),

  // -----------------------------------------------------------------------
  // 9. Apply lens
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/lenses/:lens_id/apply", async ({ params }) => {
    await delay(LATENCY_MS);
    const lensId = params["lens_id"] as string;
    return HttpResponse.json({
      ...lensResultFixture,
      lens_id: lensId,
    });
  }),

  // -----------------------------------------------------------------------
  // 10. List exploration sessions
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/explorations", async ({ params }) => {
    await delay(LATENCY_MS);
    const workspaceId = params["workspace_id"] as string | undefined;
    return HttpResponse.json([
      {
        ...explorationSessionFixture,
        workspace_id: workspaceId ?? explorationSessionFixture.workspace_id,
      },
    ]);
  }),

  // -----------------------------------------------------------------------
  // 10b. Save exploration session (ADR-040 Wave 3 — Exploration Snapshot)
  // -----------------------------------------------------------------------
  // Persists the full pane-stack exploration including viewport state.
  // Returns a session id used for ?exploration=<id> restore URLs.

  http.post("/api/exploration-sessions", async ({ request }) => {
    await delay(LATENCY_MS);
    const body = (await request.json()) as Record<string, unknown>;
    const id = `session-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
    const session = {
      id,
      workspace_id: body["workspace_id"],
      events: body["events"] ?? [],
      navigation_mode: body["navigation_mode"] ?? "pane-stack",
      panes: body["panes"] ?? [],
      created_at: new Date().toISOString(),
    };
    explorationSessionStore.set(id, session);
    return HttpResponse.json(session);
  }),

  http.get("/api/exploration-sessions/:id", async ({ params }) => {
    await delay(LATENCY_MS);
    const id = String(params["id"] ?? "");
    const session = explorationSessionStore.get(id);
    if (!session) {
      return HttpResponse.json({ error: "not_found" }, { status: 404 });
    }
    return HttpResponse.json(session);
  }),

  // -----------------------------------------------------------------------
  // 11. Generate artifact
  // -----------------------------------------------------------------------
  http.post("*/api/exploration-sessions/:exploration_id/artifacts", async ({ request }) => {
    await delay(LATENCY_MS);
    const body = (await request.json()) as { format: string };
    return HttpResponse.json({
      ...decisionArtifactFixture,
      format: body.format,
    });
  }),

  // -----------------------------------------------------------------------
  // 11b. Runtime ViewSpecs
  // -----------------------------------------------------------------------
  http.get("/api/viewspecs", async () => {
    await delay(LATENCY_MS);
    return HttpResponse.json([]);
  }),

  // -----------------------------------------------------------------------
  // 12. Subgraph (visualization-stack Phase 1)
  // -----------------------------------------------------------------------
  // The id prefix picks the fixture: `small*`, `medium*`, `large*`.
  // Anything else returns the small fixture so the rest of the UI
  // has something to render.
  http.get("/api/graph/:id/subgraph", async ({ params }) => {
    await delay(LATENCY_MS);
    const id = String(params["id"] ?? "");
    if (id.startsWith("missing")) {
      return HttpResponse.json(
        { error: "symbol_not_found" },
        { status: 404 },
      );
    }
    const fixture = id.startsWith("large")
      ? largeSubgraphFixture
      : id.startsWith("medium")
        ? mediumSubgraphFixture
        : smallSubgraphFixture;
    return HttpResponse.json({ ...fixture, root: id });
  }),

  // -----------------------------------------------------------------------
  // 14. Rationale Graph (corroboration-rationale-views)
  // -----------------------------------------------------------------------
  http.get("/api/graph/:id/rationale", async ({ params }) => {
    await delay(LATENCY_MS);
    const id = String(params["id"] ?? "");
    if (id.startsWith("missing")) {
      return HttpResponse.json(
        { error: "symbol_not_found" },
        { status: 404 },
      );
    }
    return HttpResponse.json({
      ...rationaleSubgraphFixture,
      root: id,
    });
  }),

  // -----------------------------------------------------------------------
  // 13. Contextual Graph (Contextual Views — visualization-stack Phase 2)
  // -----------------------------------------------------------------------
  // Returns a hand-rolled `ContextualGraphResponse` fixture for any
  // non-`missing*` id. `missing*` returns 404 (used by the
  // useContextualGraph error-path test).
  http.get("/api/graph/:id/contextual", async ({ params, request }) => {
    await delay(LATENCY_MS);
    const id = String(params["id"] ?? "");
    if (id.startsWith("missing")) {
      return HttpResponse.json(
        { error: "symbol_not_found" },
        { status: 404 },
      );
    }
    const url = new URL(request.url);
    const maxNodes = Number(url.searchParams.get("max_nodes") ?? "200");
    const truncated = id.startsWith("large") || id.startsWith("truncated");
    const childCount = truncated ? Math.min(250, maxNodes) : 3;
    return HttpResponse.json({
      focusNode: {
        id,
        label: "alpha",
        kind: "function",
        file: "src/alpha.rs",
        line: 1,
        style_class: "function",
      },
      parent: {
        node: {
          id: "file:src/alpha.rs",
          label: "src/alpha.rs",
          kind: "file",
          file: "src/alpha.rs",
          // `line` is omitted on the wire (Rust `Option<u32>` with
          // `skip_serializing_if`); the zod schema treats it as
          // optional. Keeping the key absent keeps the response
          // shape consistent with the rest of the explorer.
          style_class: "module",
        },
        edge: {
          source: id,
          target: "file:src/alpha.rs",
          relation: "lives_in",
          style_class: "edge.calls",
        },
      },
      children: {
        nodes: Array.from({ length: childCount }, (_, i) => ({
          id: `${id}:sib${i}`,
          label: `sib${i}`,
          kind: "function",
          file: "src/alpha.rs",
          line: 10 + i,
          style_class: "function",
        })),
        edges: Array.from({ length: childCount }, (_, i) => ({
          source: `${id}:sib${i}`,
          target: id,
          relation: "lives_in",
          style_class: "edge.calls",
        })),
      },
      sameLevel: {
        nodes: [
          {
            id: `${id}:neighbor1`,
            label: "neighbor1",
            kind: "function",
            file: "src/alpha.rs",
            line: 50,
            style_class: "function",
          },
        ],
        edges: [
          {
            source: id,
            target: `${id}:neighbor1`,
            relation: "calls",
            style_class: "edge.calls",
          },
        ],
      },
      level: "file",
      truncated,
      truncatedReason: truncated ? "max_nodes_exceeded" : null,
    });
  }),

  // -----------------------------------------------------------------------
  // Landing Page (E4 ADR-039)
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/landing", async ({ params }) => {
    await delay(LATENCY_MS);
    const workspaceId = params["workspace_id"] as string | undefined;
    if (!workspaceId) {
      return HttpResponse.json({ error: "workspace_id required" }, { status: 400 });
    }
    const { landingFixture } = await import("./landingFixtures");
    return HttpResponse.json({
      ...landingFixture,
      workspace: {
        ...landingFixture.workspace,
        id: workspaceId,
      },
    });
  }),

  // -----------------------------------------------------------------------
  // Architecture View — E5 ADR-039 (Perspective Toggle Graph ↔ C4)
  // -----------------------------------------------------------------------
  http.get("/api/workspaces/:workspace_id/architecture", async ({ params }) => {
    await delay(LATENCY_MS);
    const workspaceId = params["workspace_id"] as string | undefined;
    if (!workspaceId) {
      return HttpResponse.json({ error: "workspace_id required" }, { status: 400 });
    }
    return HttpResponse.json({
      ...architectureFixture,
    });
  }),
];
