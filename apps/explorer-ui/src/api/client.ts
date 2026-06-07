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
import type { z } from "zod";

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
// SWR-compatible fetcher factory
// ============================================================================

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
