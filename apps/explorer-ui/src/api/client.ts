/**
 * Fetch wrapper for the cognicode-explorer backend.
 *
 * All requests go through `apiGet` / `apiPost` which:
 * - Resolve a base URL from `VITE_API_BASE_URL` or fall back to `/api`
 *   (the dev server proxies `/api/*` to `127.0.0.1:8080`).
 * - Throw `ApiError` on non-2xx responses, carrying status + message.
 * - Run the parsed JSON through a zod schema at the boundary so
 *   downstream code only sees typed data.
 *
 * The boundary is the only place we trust the wire. If the backend
 * drifts, schemas fail loudly here — the rest of the app stays sound.
 */
import { z } from "zod";
import * as schemas from "./schemas";

/**
 * `VITE_API_BASE_URL` overrides the proxy in production builds.
 * The default is `/api` so the dev-server proxy and a same-origin
 * production deploy both work without config.
 */
const DEFAULT_BASE = "/api";

export function getApiBaseUrl(): string {
  const envBase = import.meta.env.VITE_API_BASE_URL as string | undefined;
  return envBase && envBase.length > 0 ? envBase : DEFAULT_BASE;
}

// ============================================================================
// Error type
// ============================================================================

export class ApiError extends Error {
  readonly status: number;
  readonly url: string;
  /** Server-supplied detail (best-effort; may be undefined). */
  readonly detail?: string;

  constructor(opts: {
    message: string;
    status: number;
    url: string;
    detail?: string;
  }) {
    super(opts.message);
    this.name = "ApiError";
    this.status = opts.status;
    this.url = opts.url;
    this.detail = opts.detail;
  }
}

// ============================================================================
// Error body parser
// ============================================================================

type ApiErrorBody = { error?: string; message?: string };

async function extractErrorBody(response: Response): Promise<string | undefined> {
  const contentType = response.headers.get("content-type") ?? "";
  if (contentType.includes("application/json")) {
    try {
      const body = (await response.clone().json()) as ApiErrorBody;
      return body.error ?? body.message;
    } catch {
      return undefined;
    }
  }
  try {
    return await response.clone().text();
  } catch {
    return undefined;
  }
}

// ============================================================================
// Generic fetch + validate
// ============================================================================

type Method = "GET" | "POST" | "PUT" | "DELETE" | "PATCH";

type RequestOpts = {
  /** Path relative to the API base (e.g. `/workspaces/:id/spotter`). */
  path: string;
  /** Query string params, encoded into the URL. */
  query?: Record<string, string | number | boolean | null | undefined>;
  /** Body for POST/PUT — serialised as JSON. */
  body?: unknown;
};

/**
 * Build a fully-qualified URL for a request, applying the base and
 * any query params. Exposed for tests and the SWR hooks so the
 * `fetcher` is a tiny one-liner.
 */
export function buildUrl(
  base: string,
  path: string,
  query?: RequestOpts["query"],
): string {
  // Tolerate either a base with or without trailing slash.
  const cleanBase = base.endsWith("/") ? base.slice(0, -1) : base;
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  if (!query) {
    return `${cleanBase}${cleanPath}`;
  }
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === null || value === undefined) continue;
    params.append(key, String(value));
  }
  const qs = params.toString();
  return qs ? `${cleanBase}${cleanPath}?${qs}` : `${cleanBase}${cleanPath}`;
}

async function request(
  method: Method,
  { path, query, body }: RequestOpts,
  base: string = getApiBaseUrl(),
): Promise<Response> {
  const url = buildUrl(base, path, query);
  const init: RequestInit = {
    method,
    headers: body !== undefined ? { "content-type": "application/json" } : {},
  };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
  }
  return fetch(url, init);
}

/**
 * Generic GET that validates the parsed JSON against a zod schema.
 * Throws `ApiError` on non-2xx; throws a `ZodError` if the response
 * does not match the schema.
 */
export async function apiGet<T extends z.ZodTypeAny>(
  path: string,
  schema: T,
  query?: RequestOpts["query"],
): Promise<z.infer<T>> {
  const response = await request("GET", { path, query });
  return handleResponse(response, schema);
}

/**
 * POST variant. Body is serialised as JSON; response is validated
 * against the schema.
 */
export async function apiPost<T extends z.ZodTypeAny>(
  path: string,
  body: unknown,
  schema: T,
): Promise<z.infer<T>> {
  const response = await request("POST", { path, body });
  return handleResponse(response, schema);
}

/**
 * PUT variant. Body is serialised as JSON; response is validated
 * against the schema.
 */
export async function apiPut<T extends z.ZodTypeAny>(
  path: string,
  body: unknown,
  schema: T,
): Promise<z.infer<T>> {
  const response = await request("PUT", { path, body });
  return handleResponse(response, schema);
}

/**
 * DELETE variant. Response is validated against the schema.
 */
export async function apiDelete<T extends z.ZodTypeAny>(
  path: string,
  schema: T,
  query?: RequestOpts["query"],
): Promise<z.infer<T>> {
  const response = await request("DELETE", { path, query });
  return handleResponse(response, schema);
}

async function handleResponse<T extends z.ZodTypeAny>(
  response: Response,
  schema: T,
): Promise<z.infer<T>> {
  if (!response.ok) {
    const detail = await extractErrorBody(response);
    throw new ApiError({
      message: detail ?? `Request failed: ${response.status} ${response.statusText}`,
      status: response.status,
      url: response.url,
      detail,
    });
  }
  // 204 No Content — return as `undefined` (caller's schema should be `z.undefined()`).
  if (response.status === 204) {
    return schema.parse(undefined) as z.infer<T>;
  }
  const json = await response.json();
  return schema.parse(json) as z.infer<T>;
}

// ============================================================================
// Subgraph (visualization-stack Phase 1)
// ============================================================================

/**
 * Subgraph query parameters. Matches `crates/cognicode-explorer/src/api.rs::SubgraphQuery`.
 *
 * All fields are optional on the wire; the backend applies defaults
 * (depth=3, direction=both, max_nodes=500) when a key is missing.
 */
export type SubgraphQuery = {
  depth?: number;
  direction?: "incoming" | "outgoing" | "both";
  max_nodes?: number;
};

/**
 * Fetch a sub-graph for the given root id. Returns the typed
 * `SubgraphResponse` (zod-validated at the boundary) or throws
 * `ApiError` on non-2xx.
 */
export async function fetchSubgraph(
  id: string,
  params: SubgraphQuery,
): Promise<import("./types").SubgraphResponse> {
  const query: Record<string, string | number> = {};
  if (params.depth !== undefined) query["depth"] = params.depth;
  if (params.direction !== undefined) query["direction"] = params.direction;
  if (params.max_nodes !== undefined) query["max_nodes"] = params.max_nodes;
  // Re-use the existing `apiGet` so the path benefits from the same
  // error + zod validation pipeline as every other DTO endpoint.
  const { subgraphResponseSchema } = await import("./types");
  return apiGet(`/graph/${encodeURIComponent(id)}/subgraph`, subgraphResponseSchema, query);
}

// ============================================================================
// Contextual Graph — Contextual Views (Phase 1 of visualization-stack)
// ============================================================================

/**
 * Options for `fetchContextual`. All fields are optional; the
 * backend applies defaults (`level=file`, `depth=1`, `max_nodes=200`).
 *
 * Mirrors `crates/cognicode-explorer/src/api::ContextualQuery`.
 */
export type ContextualOptions = {
  level?: "file";
  depth?: 1 | 2;
  maxNodes?: number;
};

/**
 * Fetch a `ContextualGraphResponse` for the given focus id. The
 * response is zod-validated at the boundary; non-2xx responses
 * surface as `ApiError` (with `status: 404` for unknown symbols,
 * `status: 400` for out-of-bounds params).
 */
export async function fetchContextual(
  id: string,
  opts: ContextualOptions = {},
): Promise<import("./types").ContextualGraphResponse> {
  const query: Record<string, string | number> = {};
  if (opts.level !== undefined) query["level"] = opts.level;
  if (opts.depth !== undefined) query["depth"] = opts.depth;
  if (opts.maxNodes !== undefined) query["max_nodes"] = opts.maxNodes;
  const { contextualGraphResponseSchema } = await import("./types");
  return apiGet(
    `/graph/${encodeURIComponent(id)}/contextual`,
    contextualGraphResponseSchema,
    query,
  );
}

// ============================================================================
// graph_search (T22) — multimodal Generic Graph Layer search
// ============================================================================
//
// Frontend-side wrapper for the MCP `graph_search` tool. The
// tool is exposed via the explorer's MCP server; the HTTP layer
// here is a thin shim that calls the same backend. The wire
// shape mirrors the Rust `dispatch_graph_search` envelope:
//
//   POST /api/mcp/tools/call { name: "graph_search", args: {...} }
//   → { payload: { results, total_count, next_cursor, ... } }
//
// For T22 we expose a typed `graphSearch` helper that:
//   - takes the user's search params (query, kinds, limit, cursor)
//   - posts to the MCP endpoint
//   - validates the response with the `graphSearchResponseSchema`
//   - returns the typed `GraphSearchResponse`
//
// The function is intentionally a thin wrapper — all the
// real work (FTS5 ranking, score normalization, cursor
// pagination) is on the Rust side. The frontend just glues
// the SWR hook to the wire.

/**
 * Parameters for the multimodal `graph_search` call.
 * Mirrors the Rust `GraphSearchArgs` struct in
 * `crates/cognicode-explorer/src/mcp.rs`.
 */
export type GraphSearchParams = {
  /** Required, non-empty search query. */
  query: string;
  /** Optional kind filter (one or more of `symbol`, `decision`, `doc`, `issue`, `evidence`). */
  node_kinds?: string[];
  /** Opaque cursor from a previous response's `next_cursor`. */
  cursor?: string;
  /** Page size; defaults to 50, capped at 200. */
  limit?: number;
};

/**
 * Run a multimodal `graph_search` and return the typed
 * `GraphSearchResponse`. Throws `ApiError` on non-2xx and
 * `ZodError` on a malformed payload.
 *
 * The MCP `tools/call` endpoint returns a `McpResultEnvelope`
 * whose `payload` is the `GraphSearchResponse` shape. We unwrap
 * the envelope in-flight and validate the inner payload.
 */
export async function graphSearch(
  params: GraphSearchParams,
): Promise<import("./types").GraphSearchResponse> {
  const { graphSearchResponseSchema } = await import("./types");
  // Use a generic schema that accepts the envelope, then unwrap.
  const envelopeSchema = await import("zod").then((z) =>
    z.z.object({
      tool_name: z.z.string(),
      payload: graphSearchResponseSchema,
    }),
  );
  const envelope = await apiPost(
    "/mcp/tools/call",
    { name: "graph_search", args: params },
    envelopeSchema,
  );
  return envelope.payload;
}

// ============================================================================
// Rationale — `GET /api/graph/:id/rationale`
// ============================================================================

/**
 * Options for `fetchRationale`. All fields are optional; the
 * backend applies defaults (`max_depth=3`, `max_nodes=50`).
 */
export type RationaleOptions = {
  maxDepth?: number;
  maxNodes?: number;
};

/**
 * Fetch a rationale sub-graph for the given focus id. Returns the
 * typed `SubgraphResponse` with `corroboration_scores` populated.
 */
export async function fetchRationale(
  id: string,
  opts: RationaleOptions = {},
): Promise<import("./types").SubgraphResponse> {
  const query: Record<string, string | number> = {};
  if (opts.maxDepth !== undefined) query["max_depth"] = opts.maxDepth;
  if (opts.maxNodes !== undefined) query["max_nodes"] = opts.maxNodes;
  const { subgraphResponseSchema } = await import("./types");
  return apiGet(
    `/graph/${encodeURIComponent(id)}/rationale`,
    subgraphResponseSchema,
    query,
  );
}

/**
 * Build a SWR fetcher that performs a GET + schema validation. The
 * returned function matches the `(key) => data` signature SWR uses
 * when no fetcher is registered globally.
 */
export function makeSwrFetcher<T extends z.ZodTypeAny>(
  schema: T,
  opts?: { base?: string },
) {
  return async (key: string | [string, RequestOpts["query"]]): Promise<z.infer<T>> => {
    if (Array.isArray(key)) {
      const [path, query] = key;
      return apiGet(path, schema, query);
    }
    // The optional `base` is applied to a one-off request rather than
    // the singleton fetcher — the global default (`/api`) is the
    // common case.
    if (opts?.base) {
      const url = buildUrl(opts.base, key);
      const response = await fetch(url);
      return handleResponse(response, schema);
    }
    return apiGet(key, schema);
  };
}

// ============================================================================
// ViewSpec CRUD — Phase 4 Authoring Wizard
// ============================================================================

/**
 * Request body for saving a ViewSpec.
 * The backend assigns `created_at` / `updated_at`.
 */
export interface SaveViewSpecRequest {
  workspace_id: string;
  owner: string;
  spec: import("./schemas").ViewSpec;
}

/** Response after saving a ViewSpec. */
interface SaveViewSpecResponse {
  id: string;
}

/**
 * `POST /api/viewspecs` — save (create) a ViewSpec.
 * Returns the assigned id.
 */
export async function saveViewSpec(
  request: SaveViewSpecRequest,
): Promise<SaveViewSpecResponse> {
  return apiPost(
    "/viewspecs",
    request,
    z.object({ id: z.string() }),
  );
}

/**
 * `GET /api/viewspecs` — list ViewSpecs for (workspace_id, owner).
 */
export async function listViewSpecs(
  workspaceId: string,
  owner: string,
): Promise<schemas.ViewSpec[]> {
  return apiGet(
    "/viewspecs",
    z.array(schemas.viewSpecSchema),
    { workspace_id: workspaceId, owner },
  );
}

/**
 * `GET /api/viewspecs/:id` — load a single ViewSpec.
 */
export async function loadViewSpec(
  id: string,
  workspaceId: string,
  owner: string,
): Promise<schemas.ViewSpec> {
  return apiGet(
    `/viewspecs/${encodeURIComponent(id)}`,
    schemas.viewSpecSchema,
    { workspace_id: workspaceId, owner },
  );
}

/**
 * `PUT /api/viewspecs/:id` — replace a ViewSpec.
 */
export async function updateViewSpec(
  id: string,
  request: Omit<SaveViewSpecRequest, "workspace_id" | "owner">,
): Promise<SaveViewSpecResponse> {
  return apiPut(
    `/viewspecs/${encodeURIComponent(id)}`,
    request,
    z.object({ id: z.string() }),
  );
}

/**
 * `DELETE /api/viewspecs/:id` — delete a ViewSpec.
 */
export async function deleteViewSpec(
  id: string,
  workspaceId: string,
  owner: string,
): Promise<{ deleted: boolean }> {
  return apiDelete(
    `/viewspecs/${encodeURIComponent(id)}`,
    z.object({ deleted: z.boolean() }),
    { workspace_id: workspaceId, owner },
  );
}

/**
 * `POST /api/viewspecs/execute` — execute a ViewSpec against an object.
 * Returns a `ContextualView` ready for rendering via `rendererRegistry`.
 */
export async function executeViewSpec(
  spec: schemas.ViewSpec,
  objectId: string,
): Promise<schemas.ContextualView> {
  return apiPost(
    "/viewspecs/execute",
    { spec, object_id: objectId },
    schemas.contextualViewSchema,
  );
}

// ============================================================================
// Landing Page — E4 ADR-039
// ============================================================================

/**
 * Fetch the landing page payload for a workspace.
 * Returns workspace summary, graph nodes/edges, entry points, hot paths,
 * god nodes, and suggested questions.
 */
export async function fetchLanding(
  workspaceId: string,
): Promise<import("./types").LandingPayload> {
  return apiGet(
    `/workspaces/${encodeURIComponent(workspaceId)}/landing`,
    schemas.landingPayloadSchema,
  );
}

// Architecture View — E5 ADR-039 (Perspective Toggle Graph ↔ C4)
// ============================================================================

/**
 * Fetch the C4 component architecture for a workspace.
 * Returns a SubgraphResponse with component nodes (directories) and
 * part_of edges reflecting directory hierarchy.
 */
export async function fetchArchitecture(
  workspaceId: string,
): Promise<import("./types").ArchitecturePayload> {
  return apiGet(
    `/workspaces/${encodeURIComponent(workspaceId)}/architecture`,
    schemas.architecturePayloadSchema,
  );
}
