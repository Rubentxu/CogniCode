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
  explorationPathFixture,
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

const LATENCY_MS = 8;

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
    // Echo the query back via the match_type so the UI can confirm.
    return HttpResponse.json(
      spotterResultsFixture.map((hit, i) => ({
        ...hit,
        match_type: i === 0 ? `query:${q}` : hit.match_type,
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
    return HttpResponse.json({
      ...contextualViewFixture,
      object_id: objectId,
      view_id: viewId,
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
  // 10. Save exploration
  // -----------------------------------------------------------------------
  http.post("/api/explorations", async ({ request }) => {
    await delay(LATENCY_MS);
    const body = (await request.json()) as { workspace_id: string };
    return HttpResponse.json({
      ...explorationPathFixture,
      workspace_id: body.workspace_id,
    });
  }),

  // -----------------------------------------------------------------------
  // 11. Generate artifact
  // -----------------------------------------------------------------------
  http.post("/api/explorations/:exploration_id/artifacts", async ({ request }) => {
    await delay(LATENCY_MS);
    const body = (await request.json()) as { format: string };
    return HttpResponse.json({
      ...decisionArtifactFixture,
      format: body.format,
    });
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
      truncationReason: truncated ? "max_nodes_exceeded" : null,
    });
  }),
];
