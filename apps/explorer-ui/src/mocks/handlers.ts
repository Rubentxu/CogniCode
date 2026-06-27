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
  inspectableScopeFixture,
  lensDescriptorsFixture,
  lensResultFixture,
  spotterResultsFixture,
  workspaceSummaryFixture,
  // e12a–e12e Phase 1 executor fixtures
  usageExamplesViewFixture,
  apiSurfaceViewFixture,
  testSliceViewFixture,
  debugSliceViewFixture,
  changeImpactStoryViewFixture,
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
    // e12a–e12e Phase 1 executors
    case "usage-examples":
      return { viewKind: "usage_examples", rendererKind: "table" };
    case "api-surface":
      return { viewKind: "api_surface", rendererKind: "table" };
    case "test-slice":
      return { viewKind: "test_slice", rendererKind: "table" };
    case "debug-slice":
      return { viewKind: "debug_slice", rendererKind: "graph" };
    case "change-impact-story":
      return { viewKind: "change_impact_story", rendererKind: "table" };
    case "overview":
    default:
      return { viewKind: "vertical_slice", rendererKind: "composite" };
  }
}

// In-memory store for exploration sessions (ADR-040 Wave 3 H4 fix).
// Tests rely on this to validate session save/restore round-trip.
export const explorationSessionStore = new Map<string, Record<string, unknown>>();

// e15.5 — In-memory store for ingested OpenAPI routes.
// After ingest_openapi populates this, the Spotter and Landing
// handlers include the route nodes in their responses so the UI
// can display them.
export interface MockRoute {
  id: string;
  method: string;
  path: string;
  protocol: string;
  handler_symbol: string | null;
  spec_source: string;
  spec_hash: string;
  framework: string | null;
  confidence: number;
  properties: Record<string, unknown>;
}
export const routeStore = new Map<string, MockRoute>();

// e15.5 — reset endpoint to clear routeStore between tests
http.post("/api/mocks/reset", async () => {
  routeStore.clear();
  explorationSessionStore.clear();
  return HttpResponse.json({ ok: true });
});

// Petstore fixture SHA256 — used by both ingest_openapi and trace_route handlers
const PETSTORE_HASH = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

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
    const symbolResults = spotterResultsFixture.map((hit, i) => ({
      kind: "symbol",
      result: {
        ...hit,
        match_type: i === 0 ? `query:${q}` : hit.match_type,
      },
    }));

    // e15.5: include ingested Route nodes when query matches their path or method
    // Route nodes are added to the store by the ingest_openapi handler.
    const qLower = q.toLowerCase();
    const routeResults = Array.from(routeStore.values())
      .filter((r) => {
        const pathMatch = r.path.toLowerCase().includes(qLower);
        const methodMatch = r.method.toLowerCase().includes(qLower);
        const handlerMatch = (r.handler_symbol ?? "").toLowerCase().includes(qLower);
        const summary = (r.properties["summary"] as string ?? "").toLowerCase();
        return pathMatch || methodMatch || handlerMatch || summary.includes(qLower);
      })
      .map((route) => ({
        kind: "route" as const,
        result: {
          object: {
            id: route.id,
            object_type: "route",
            label: `${route.method} ${route.path}`,
            subtitle: route.handler_symbol ?? "unresolved",
            properties: [
              { key: "method", value: route.method, value_type: "string", source: "static" },
              { key: "path", value: route.path, value_type: "string", source: "static" },
              { key: "protocol", value: route.protocol, value_type: "string", source: "static" },
              { key: "handler", value: route.handler_symbol ?? "unresolved", value_type: "string", source: "static" },
              { key: "confidence", value: route.confidence, value_type: "number", source: "static" },
            ],
            available_views: [
              { id: "overview", title: "Overview", is_builtin: true, source: null },
              { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
            ],
          },
          score: 0.95,
          match_type: `query:${q}`,
        },
      }));

    return HttpResponse.json([...symbolResults, ...routeResults]);
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

    // e15.5: handle Route node inspection
    if (objectId.startsWith("route:")) {
      const route = routeStore.get(objectId);
      if (!route) {
        return HttpResponse.json(
          { error: `Route not found: ${objectId}` },
          { status: 404 },
        );
      }
      return HttpResponse.json({
        id: route.id,
        object_type: "route",
        label: `${route.method} ${route.path}`,
        subtitle: route.handler_symbol ?? "unresolved",
        properties: [
          { key: "method", value: route.method, value_type: "string", source: "static" },
          { key: "path", value: route.path, value_type: "string", source: "static" },
          { key: "protocol", value: route.protocol, value_type: "string", source: "static" },
          { key: "framework", value: route.framework ?? "—", value_type: "string", source: "static" },
          { key: "handler", value: route.handler_symbol ?? "unresolved", value_type: "string", source: "static" },
          { key: "confidence", value: route.confidence, value_type: "number", source: "static" },
          { key: "spec_source", value: route.spec_source, value_type: "string", source: "static" },
        ],
          available_views: [
          { id: "overview", title: "Overview", is_builtin: true, source: null },
          { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
        ],
      });
    }

    // e12b: handle Scope node inspection
    if (objectId?.startsWith("scope:")) {
      return HttpResponse.json(inspectableScopeFixture);
    }

    return HttpResponse.json({
      ...inspectableObjectFixture,
      id: objectId,
    });
  }),

  // -----------------------------------------------------------------------
  // 6. Available views
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/views", async ({ params }) => {
    await delay(LATENCY_MS);
    const objectId = params["object_id"] as string | undefined;
    if (objectId?.startsWith("route:")) {
      return HttpResponse.json([
        { id: "overview", title: "Overview", is_builtin: true, source: null },
        { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
      ]);
    }
    if (objectId?.startsWith("scope:")) {
      return HttpResponse.json(inspectableScopeFixture.available_views);
    }
    return HttpResponse.json(inspectableObjectFixture.available_views);
  }),

  // -----------------------------------------------------------------------
  // 7. Contextual view
  // -----------------------------------------------------------------------
  http.get("/api/objects/:object_id/views/:view_id", async ({ params }) => {
    await delay(LATENCY_MS);
    const objectId = params["object_id"] as string;
    const viewId = params["view_id"] as string;

    // e15.5: route-specific contextual views
    if (objectId.startsWith("route:")) {
      const route = routeStore.get(objectId);
      if (!route) {
        return HttpResponse.json({ error: `Route not found: ${objectId}` }, { status: 404 });
      }

      if (viewId === "overview") {
        return HttpResponse.json({
          object_id: objectId,
          view_id: "overview",
          title: `${route.method} ${route.path}`,
          view_kind: "vertical_slice",
          renderer_kind: "composite",
          blocks: [
            {
              id: "route_identity",
              title: "Route Identity",
              body: {
                method: route.method,
                path: route.path,
                protocol: route.protocol,
                spec_source: route.spec_source,
              },
            },
            {
              id: "route_handler",
              title: "Handler Symbol",
              body: {
                symbol: route.handler_symbol ?? "unresolved",
                confidence: route.confidence,
                operation_id: route.properties["operation_id"],
                summary: route.properties["summary"],
              },
            },
          ],
          relations: [],
          evidence: [],
          findings: [],
        });
      }

      if (viewId === "call-graph") {
        // Route call-graph shows the HTTP_calls edge from route → handler
        const handlerId = route.handler_symbol
          ? `symbol:${route.handler_symbol}`
          : null;
        return HttpResponse.json({
          object_id: objectId,
          view_id: "call-graph",
          title: `${route.method} ${route.path} → ${route.handler_symbol ?? "unresolved"}`,
          view_kind: "call_graph",
          renderer_kind: "graph",
          blocks: [],
          relations: handlerId
            ? [
                {
                  source: objectId,
                  target: handlerId,
                  relation: "http_calls",
                  style_class: "edge.http_calls",
                },
              ]
            : [],
          evidence: [],
          findings: [],
        });
      }
    }

    const { viewKind, rendererKind } = viewIdToKinds(viewId);

    // e12a–e12e: Phase 1 executor fixtures
    switch (viewId) {
      case "usage-examples":
        return HttpResponse.json({ ...usageExamplesViewFixture, object_id: objectId });
      case "api-surface":
        return HttpResponse.json({ ...apiSurfaceViewFixture, object_id: objectId });
      case "test-slice":
        return HttpResponse.json({ ...testSliceViewFixture, object_id: objectId });
      case "debug-slice":
        return HttpResponse.json({ ...debugSliceViewFixture, object_id: objectId });
      case "change-impact-story":
        return HttpResponse.json({ ...changeImpactStoryViewFixture, object_id: objectId });
    }

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

    // e15.5: Route contextual graph — shows the route + its handler symbol
    if (id.startsWith("route:")) {
      const route = routeStore.get(id);
      if (!route) {
        return HttpResponse.json({ error: "symbol_not_found" }, { status: 404 });
      }
      const handlerId = route.handler_symbol
        ? `symbol:${route.handler_symbol}`
        : null;
      const childrenNodes = handlerId
        ? [
            {
              id: handlerId,
              label: route.handler_symbol!,
              kind: "symbol",
              file: route.spec_source,
              line: null,
              style_class: "function",
            },
          ]
        : [];
      const childrenEdges = handlerId
        ? [
            {
              source: id,
              target: handlerId,
              relation: "http_calls",
              style_class: "edge.http_calls",
            },
          ]
        : [];
      return HttpResponse.json({
        focusNode: {
          id: route.id,
          label: `${route.method} ${route.path}`,
          kind: "route",
          file: route.spec_source,
          line: null,
          style_class: "route",
        },
        parent: null,
        children: {
          nodes: childrenNodes,
          edges: childrenEdges,
        },
        sameLevel: { nodes: [], edges: [] },
        level: "route",
        truncated: false,
        truncatedReason: null,
      });
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

    // e15.5: include ingested Route nodes as graph nodes + edges
    const routeNodes = Array.from(routeStore.values()).map((route) => ({
      id: route.id,
      label: `${route.method} ${route.path}`,
      kind: "route" as const,
      file: route.spec_source,
      line: null,
      style_class: "route" as const,
    }));
    const routeEdges = Array.from(routeStore.values())
      .filter((r) => r.handler_symbol != null)
      .map((route) => ({
        source: route.id,
        target: `symbol:${route.handler_symbol}`,
        relation: "http_calls" as const,
        style_class: "edge.http_calls" as const,
      }));

    return HttpResponse.json({
      ...landingFixture,
      workspace: {
        ...landingFixture.workspace,
        id: workspaceId,
      },
      nodes: [...landingFixture.nodes, ...routeNodes],
      edges: [...landingFixture.edges, ...routeEdges],
      entry_points: routeNodes.length > 0
        ? [
            ...landingFixture.entry_points,
            ...routeNodes.map((node) => ({
              id: node.id,
              object_type: "route" as const,
              label: node.label,
              subtitle: (routeStore.get(node.id)?.handler_symbol) ?? "unresolved",
              properties: [
                { key: "kind", value: "route", value_type: "string", source: "static" },
              ],
              available_views: [
                { id: "overview", title: "Overview", is_builtin: true, source: null },
                { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
              ],
            })),
          ]
        : landingFixture.entry_points,
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

  // -------------------------------------------------------------------------
  // e15.5 — MCP Tools (OpenAPI / gRPC / GraphQL / trPC ingestion)
  // -------------------------------------------------------------------------
  //
  // The MCP HTTP transport accepts POST /mcp/tools/call with
  //   { name: "ingest_openapi" | "trace_route", args: {...} }
  // and returns a McpResultEnvelope whose payload is the tool result.
  //
  // Ingestion of the petstore.json fixture produces 7 routes:
  //   GET  /pets, POST /pets, GET /pets/{petId}, PUT /pets/{petId},
  //   DELETE /pets/{petId}, GET /store/inventory, POST /store/order
  //
  // Subsequent calls with the same spec hash return "already_ingested".
  // -------------------------------------------------------------------------

  http.post("/api/mcp/tools/call", async ({ request }) => {
    await delay(LATENCY_MS);
    const body = (await request.clone().json()) as {
      name?: string;
      args?: Record<string, unknown>;
    };
    const toolName = body.name;
    const args = body.args ?? {};

    if (toolName === "ingest_openapi") {
      const spec = String(args["spec"] ?? "");

      // Idempotency: if the store already has routes for this hash, return already_ingested
      const existingRoutes = Array.from(routeStore.values()).filter(
        (r) => r.spec_hash === PETSTORE_HASH,
      );
      if (existingRoutes.length > 0) {
        return HttpResponse.json({
          tool_name: "ingest_openapi",
          version: "0.0.0",
          timestamp: new Date().toISOString(),
          provenance: null,
          payload: {
            spec_hash: PETSTORE_HASH,
            status: "already_ingested",
            routes_count: existingRoutes.length,
            message: "Spec hash already ingested; no changes detected. Delete existing routes to re-ingest.",
          },
        });
      }

      if (!spec) {
        return HttpResponse.json({
          tool_name: "ingest_openapi",
          version: "0.0.0",
          timestamp: new Date().toISOString(),
          provenance: null,
          payload: { error: "spec path must be non-empty" },
        }, { status: 400 });
      }

      // Populate routeStore with the 7 petstore routes
      const PETSTORE_ROUTES: MockRoute[] = [
        { id: "route:HTTP:GET:/pets",        method: "GET",    path: "/pets",           protocol: "http", handler_symbol: "list_pets",       spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum",     confidence: 0.85, properties: { operation_id: "listPets",       summary: "List all pets" } },
        { id: "route:HTTP:POST:/pets",       method: "POST",   path: "/pets",           protocol: "http", handler_symbol: "create_pet",      spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum",     confidence: 0.85, properties: { operation_id: "createPet",       summary: "Create a new pet" } },
        { id: "route:HTTP:GET:/pets/{petId}", method: "GET",    path: "/pets/{petId}",  protocol: "http", handler_symbol: "get_pet_by_id",  spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum",     confidence: 0.85, properties: { operation_id: "getPetById",     summary: "Find pet by ID" } },
        { id: "route:HTTP:PUT:/pets/{petId}", method: "PUT",    path: "/pets/{petId}",  protocol: "http", handler_symbol: "update_pet",     spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum",     confidence: 0.85, properties: { operation_id: "updatePet",     summary: "Update a pet" } },
        { id: "route:HTTP:DELETE:/pets/{petId}", method: "DELETE", path: "/pets/{petId}", protocol: "http", handler_symbol: "delete_pet", spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum", confidence: 0.85, properties: { operation_id: "deletePet", summary: "Delete a pet" } },
        { id: "route:HTTP:GET:/store/inventory", method: "GET", path: "/store/inventory", protocol: "http", handler_symbol: "get_inventory", spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum", confidence: 0.85, properties: { operation_id: "getInventory", summary: "Get store inventory" } },
        { id: "route:HTTP:POST:/store/order",   method: "POST", path: "/store/order",    protocol: "http", handler_symbol: "place_order",   spec_source: spec, spec_hash: PETSTORE_HASH, framework: "axum", confidence: 0.85, properties: { operation_id: "placeOrder",   summary: "Place an order for a pet" } },
      ];
      for (const route of PETSTORE_ROUTES) {
        routeStore.set(route.id, route);
      }

      return HttpResponse.json({
        tool_name: "ingest_openapi",
        version: "0.0.0",
        timestamp: new Date().toISOString(),
        provenance: null,
        payload: {
          spec_hash: PETSTORE_HASH,
          status: "ingested",
          routes_created: PETSTORE_ROUTES.length,
          routes_updated: 0,
          edges_created: 4,
          edges_updated: 0,
          total_routes: PETSTORE_ROUTES.length,
          resolved_handlers: PETSTORE_ROUTES.filter((r) => r.handler_symbol != null).length,
          framework: args["framework"] ?? null,
        },
      });
    }

    if (toolName === "trace_route") {
      const method = String(args["method"] ?? "").toUpperCase();
      const path = String(args["path"] ?? "");

      // Look up the route from routeStore (populated by ingest_openapi).
      // Supports exact path match + wildcard match for paths with `{paramName}`.
      const candidates = Array.from(routeStore.values()).filter((r) => r.method === method);
      let matchedRoute: MockRoute | undefined;
      let matchedPattern: string | undefined;

      // Try exact match first
      matchedRoute = candidates.find((r) => r.path === path);
      if (matchedRoute) {
        matchedPattern = path;
      } else {
        // Try wildcard match: convert `/pets/{petId}` → regex `^/pets/[^/]+$`
        for (const route of candidates) {
          const regexPath = route.path.replace(/\{[^}]+\}/g, "[^/]+");
          const regex = new RegExp(`^${regexPath}$`);
          if (regex.test(path)) {
            matchedRoute = route;
            matchedPattern = route.path;
            break;
          }
        }
      }

      if (!matchedRoute) {
        return HttpResponse.json({
          tool_name: "trace_route",
          version: "0.0.0",
          timestamp: new Date().toISOString(),
          provenance: null,
          payload: {
            error: `no route found for ${method} ${path} (check that the spec has been ingested via \`ingest_openapi\`)`,
          },
        }, { status: 404 });
      }

      return HttpResponse.json({
        tool_name: "trace_route",
        version: "0.0.0",
        timestamp: new Date().toISOString(),
        provenance: null,
        payload: {
          route: {
            id: matchedRoute.id,
            method: matchedRoute.method,
            path: matchedRoute.path,
            protocol: matchedRoute.protocol,
            handler_symbol: matchedRoute.handler_symbol,
            spec_source: matchedRoute.spec_source,
            spec_hash: matchedRoute.spec_hash,
            framework: matchedRoute.framework,
            confidence: matchedRoute.confidence,
            properties: matchedRoute.properties,
          },
          match: {
            method_normalized: method,
            path_normalized: path,
            matched_pattern: matchedPattern,
          },
        },
      });
    }

    // Unknown tool
    return HttpResponse.json({
      tool_name: toolName ?? "unknown",
      version: "0.0.0",
      timestamp: new Date().toISOString(),
      provenance: null,
      payload: { error: `Unknown tool: ${toolName}` },
    }, { status: 400 });
  }),
];
