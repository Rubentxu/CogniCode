/**
 * `McpToolsModal` — e15.5 Cross-Service Protocol Edge Ingestion.
 *
 * Provides a minimal UI for invoking MCP tools that manage OpenAPI /
 * gRPC / GraphQL / trRPC route definitions:
 *
 *  - `ingest_openapi` — reads an OpenAPI 3.x spec from the filesystem,
 *    emits `route:HTTP:…` nodes and `http_calls` edges into the graph.
 *  - `trace_route`  — reverse-lookup a route by `(method, path)` to
 *    find the resolved handler symbol.
 *
 * The modal is deliberately minimal: tool selector → parameter form →
 * result viewer.  All network calls go through the shared `apiPost`
 * wrapper so MSW intercepts them in E2E and the real server in
 * integration / dev.
 *
 * The modal is opened by the `mcp-tools-trigger` button in the Shell
 * header.
 */
import { useState } from "react";
import { z } from "zod";
import { apiPost } from "../api/client";

// ---------------------------------------------------------------------------
// Types (mirrors Rust handler args + response)
// ---------------------------------------------------------------------------

interface IngestOpenApiArgs {
  spec: string;
  framework?: string;
}

interface TraceRouteArgs {
  method: string;
  path: string;
}

type ToolName = "ingest_openapi" | "trace_route";

interface ToolCallResult {
  tool_name: string;
  version: string;
  timestamp: string;
  provenance: unknown;
  payload: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// MCP tool registry
// ---------------------------------------------------------------------------

const TOOLS: { name: ToolName; description: string }[] = [
  {
    name: "ingest_openapi",
    description:
      "Ingest an OpenAPI 3.x spec file and emit route nodes + HTTP-edges into the graph.",
  },
  {
    name: "trace_route",
    description:
      "Look up a route by HTTP method + path and return its resolved handler symbol.",
  },
];

const HTTP_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"] as const;

// ---------------------------------------------------------------------------
// API call (uses the same /mcp/tools/call endpoint as the MCP HTTP transport)
// ---------------------------------------------------------------------------

async function callMcpTool(tool: ToolName, args: IngestOpenApiArgs | TraceRouteArgs): Promise<ToolCallResult> {
  // The MCP HTTP transport accepts { name, args } and returns the envelope.
  // We use the same `apiPost` helper the rest of the client uses, pointing
  // at the MCP endpoint.  When VITE_USE_MOCKS=true, MSW intercepts this
  // call and returns deterministic fixtures.
  // NOTE: using absolute path /src/... for Vite module resolution reliability.
  const { apiPost } = await import("/src/api/client");
  // Use z.any() schema to bypass schema.parse() in handleResponse.
  // The caller is responsible for type-checking the result.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return apiPost("/mcp/tools/call", { name: tool, args }, z.any()) as Promise<ToolCallResult>;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function McpToolsModal({
  onClose,
}: {
  onClose: () => void;
}) {
  const [selectedTool, setSelectedTool] = useState<ToolName>("ingest_openapi");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<ToolCallResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // ingest_openapi fields
  const [specPath, setSpecPath] = useState("");
  const [framework, setFramework] = useState("");

  // trace_route fields
  const [method, setMethod] = useState<typeof HTTP_METHODS[number]>("GET");
  const [path, setPath] = useState("");

  const handleRun = async () => {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      let args: IngestOpenApiArgs | TraceRouteArgs;
      if (selectedTool === "ingest_openapi") {
        args = { spec: specPath, ...(framework ? { framework } : {}) };
      } else {
        args = { method, path };
      }
      const res = await callMcpTool(selectedTool, args);
      setResult(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const canRun =
    selectedTool === "ingest_openapi"
      ? specPath.trim().length > 0
      : path.trim().length > 0;

  return (
    <div
      data-testid="mcp-tools-modal"
      role="dialog"
      aria-modal="true"
      aria-label="MCP Tools"
      className="absolute inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.45)" }}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        data-testid="mcp-tools-modal-panel"
        className="flex flex-col rounded-lg p-6 shadow-xl"
        style={{
          width: "520px",
          maxWidth: "90vw",
          maxHeight: "85vh",
          backgroundColor: "var(--color-surface-raised)",
          border: "1px solid var(--color-border)",
          overflow: "auto",
        }}
      >
        {/* Header */}
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-base font-semibold" style={{ color: "var(--color-text-primary)" }}>
            MCP Tools
          </h2>
          <button
            type="button"
            onClick={onClose}
            data-testid="mcp-tools-modal-close"
            aria-label="Close"
            className="text-sm"
            style={{ color: "var(--color-text-muted)" }}
          >
            ✕
          </button>
        </div>

        {/* Tool selector */}
        <div className="mb-4">
          <label
            htmlFor="mcp-tool-select"
            className="mb-1 block text-xs font-medium"
            style={{ color: "var(--color-text-secondary)" }}
          >
            Tool
          </label>
          <select
            id="mcp-tool-select"
            data-testid="mcp-tool-select"
            value={selectedTool}
            onChange={(e) => { setSelectedTool(e.target.value as ToolName); setResult(null); setError(null); }}
            className="w-full rounded px-3 py-2 text-sm"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
              border: "1px solid var(--color-border)",
            }}
          >
            {TOOLS.map((t) => (
              <option key={t.name} value={t.name}>
                {t.name}
              </option>
            ))}
          </select>
          <p
            className="mt-1 text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            {TOOLS.find((t) => t.name === selectedTool)?.description}
          </p>
        </div>

        {/* Parameter form */}
        {selectedTool === "ingest_openapi" && (
          <div className="mb-4 flex flex-col gap-3">
            <div>
              <label
                htmlFor="mcp-spec-path"
                className="mb-1 block text-xs font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                Spec file path <span style={{ color: "var(--color-text-error)" }}>*</span>
              </label>
              <input
                id="mcp-spec-path"
                data-testid="mcp-spec-path"
                type="text"
                value={specPath}
                onChange={(e) => setSpecPath(e.target.value)}
                placeholder="sandbox/fixtures/openapi/petstore.json"
                className="w-full rounded px-3 py-2 text-sm font-mono"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-primary)",
                  border: "1px solid var(--color-border)",
                }}
              />
            </div>
            <div>
              <label
                htmlFor="mcp-framework"
                className="mb-1 block text-xs font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                Framework hint
              </label>
              <select
                id="mcp-framework"
                data-testid="mcp-framework"
                value={framework}
                onChange={(e) => setFramework(e.target.value)}
                className="w-full rounded px-3 py-2 text-sm"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-primary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                <option value="">— none —</option>
                <option value="axum">axum</option>
                <option value="actix-web">actix-web</option>
                <option value="express">express</option>
                <option value="fastapi">fastapi</option>
              </select>
            </div>
          </div>
        )}

        {selectedTool === "trace_route" && (
          <div className="mb-4 flex flex-col gap-3">
            <div>
              <label
                htmlFor="mcp-trace-method"
                className="mb-1 block text-xs font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                HTTP Method <span style={{ color: "var(--color-text-error)" }}>*</span>
              </label>
              <select
                id="mcp-trace-method"
                data-testid="mcp-trace-method"
                value={method}
                onChange={(e) => setMethod(e.target.value as typeof HTTP_METHODS[number])}
                className="w-full rounded px-3 py-2 text-sm"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-primary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                {HTTP_METHODS.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
            </div>
            <div>
              <label
                htmlFor="mcp-trace-path"
                className="mb-1 block text-xs font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                URL Path <span style={{ color: "var(--color-text-error)" }}>*</span>
              </label>
              <input
                id="mcp-trace-path"
                data-testid="mcp-trace-path"
                type="text"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="/pets/{id}"
                className="w-full rounded px-3 py-2 text-sm font-mono"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-primary)",
                  border: "1px solid var(--color-border)",
                }}
              />
            </div>
          </div>
        )}

        {/* Run button */}
        <button
          type="button"
          data-testid="mcp-tools-run"
          onClick={handleRun}
          disabled={!canRun || loading}
          className="mb-4 rounded px-4 py-2 text-sm font-medium"
          style={{
            backgroundColor: canRun && !loading ? "var(--color-accent)" : "var(--color-surface-overlay)",
            color: canRun && !loading ? "white" : "var(--color-text-muted)",
            cursor: !canRun || loading ? "not-allowed" : "pointer",
            opacity: loading ? 0.7 : 1,
          }}
        >
          {loading ? "Running…" : "Run"}
        </button>

        {/* Error */}
        {error && (
          <div
            data-testid="mcp-tools-error"
            className="mb-4 rounded p-3 text-sm"
            style={{
              backgroundColor: "color-mix(in srgb, var(--color-text-error) 10%, transparent)",
              color: "var(--color-text-error)",
              border: "1px solid color-mix(in srgb, var(--color-text-error) 30%, transparent)",
            }}
          >
            {error}
          </div>
        )}

        {/* Result */}
        {result && (
          <div>
            <p
              className="mb-2 text-xs font-medium uppercase"
              style={{ color: "var(--color-text-muted)" }}
            >
              Result
            </p>
            <div
              data-testid="mcp-tools-result"
              className="rounded p-3 text-sm font-mono"
              style={{
                backgroundColor: "var(--color-surface-overlay)",
                color: "var(--color-text-primary)",
                border: "1px solid var(--color-border)",
                maxHeight: "240px",
                overflow: "auto",
              }}
            >
              <pre
                style={{ whiteSpace: "pre-wrap", wordBreak: "break-all" }}
              >
                {JSON.stringify(result.payload, null, 2)}
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
