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
];
