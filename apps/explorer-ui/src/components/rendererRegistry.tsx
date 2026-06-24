/**
 * `rendererRegistry` — maps `RendererKind` strings to concrete React
 * components.
 *
 * ADR-008 §RendererRegistry: The Explorer owns visual rendering. Renderer
 * ids (`graph`, `table`, `tree`, `code`, `vega_lite`, `json`, `markdown`,
 * `composite`) map to React components via this registry. The backend sends
 * ViewSpecs and data; the frontend chooses the concrete renderer via this map.
 *
 * Sprint E1: E1.1 wires `graph` to the real `InteractiveGraph` component
 * (previously was placeholder). E1.2-E1.5 will wire the remaining renderers.
 */
import type { ReactNode } from "react";

import { detectLanguage } from "../utils/languageDetect";
import { highlightCode } from "../utils/highlight";
import type { RendererKind } from "../api/schemas";
import type { ContextualView } from "../api/types";
import { GraphView } from "./GraphView/GraphView";

// ============================================================================
// Renderer id type — first-class catalog from ADR-008
// ============================================================================

/**
 * First-class renderer ids. Unknown strings are accepted at parse time
 * (forward compatibility) but fall through to the `getOrJson` fallback at
 * render time.
 */
export type RendererId = RendererKind;

// ============================================================================
// RuntimeContext — typed union of what graph and block renderers need
// ============================================================================

/**
 * Context passed through the renderer call chain.
 *
 * All fields are optional — callers provide only what the renderer needs.
 * This is a backward-compatible additive extension of the original
 * `render(body, extra?: Record<string, unknown>)` signature.
 *
 * For the graph renderer: `view` is required; `objectId`, `paneId`,
 * `onClose` are optional.
 * For block renderers: none are required.
 */
export interface RuntimeContext {
  /** The full ContextualView — used by the graph renderer entry. */
  view?: ContextualView;
  /** The object being inspected. */
  objectId?: string;
  /** Pane ID for viewport-snapshot dispatch. */
  paneId?: string;
  /** View ID for SELECT_OBJECT payload. */
  viewId?: string;
  /** Close callback — used by graph renderer for pane close button. */
  onClose?: () => void;
  /**
   * Navigate to a related object. Used by interactive block renderers
   * (callers, callees, hotspots, quality_issue_detail).
   */
  onSelectObject?: (objectId: string, viewId?: string) => void;
}

// ============================================================================
// Registry entry
// ============================================================================

export interface RendererEntry {
  /** Human-readable label shown in dev tools / error messages. */
  label: string;
  /**
   * Render function. Receives the `body` (opaque JSON from the block or
   * ViewSpec `props`) and optional `RuntimeContext`.
   * Returns a React node — caller is responsible for error boundaries.
   */
  render: (body: unknown, extra?: RuntimeContext) => ReactNode;
}

// ============================================================================
// The registry
// ============================================================================

/**
 * Global renderer registry — a `Map` from `RendererId` to `RendererEntry`.
 *
 * Initialised with the 8 built-in renderers at module load time.
 * Additional renderers can be registered at runtime (ViewSpec authoring
 * wizard, future extension host, etc.).
 */
class RendererRegistry {
  #map = new Map<RendererId | string, RendererEntry>();

  constructor() {
    this.#registerBuiltin();
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Register (or replace) a renderer for the given id.
   * Returns the entry that was previously registered (if any) so callers
   * can restore it if needed.
   */
  register(id: RendererId, entry: RendererEntry): RendererEntry | undefined {
    const prev = this.#map.get(id);
    this.#map.set(id, entry);
    return prev;
  }

  /**
   * Look up a renderer by id. Returns `undefined` if not registered.
   */
  get(id: RendererId | string): RendererEntry | undefined {
    return this.#map.get(id);
  }

  /**
   * Look up a renderer by id. If not found, falls back to the `json`
   * renderer (which renders the body as pretty-printed JSON).
   *
   * This is the SAFE render path — callers should always use this unless
   * they have a specific reason to reject unknown renderers.
   */
  getOrJson(id: RendererId | string): RendererEntry {
    return this.#map.get(id) ?? this.#getJsonRenderer();
  }

  /** Returns an iterator over all registered (id, entry) pairs. */
  entries(): IterableIterator<[RendererId | string, RendererEntry]> {
    return this.#map.entries();
  }

  /**
   * Render a body with the given renderer id, using `getOrJson` fallback.
   * Convenience wrapper — equivalent to `registry.getOrJson(id).render(body)`.
   */
  render(id: RendererId | string, body: unknown): ReactNode {
    return this.getOrJson(id).render(body);
  }

  // --------------------------------------------------------------------------
  // Built-in registration
  // --------------------------------------------------------------------------

  #registerBuiltin() {
    // `graph` — delegates to the canonical GraphView component (E1.5).
    // GraphView gets dispatch from useAppDispatch() internally.
    // extra.view is required (the full ContextualView); objectId, paneId,
    // onClose are optional.
    this.register("graph", {
      label: "Graph",
      render: (body, extra) => {
        const view = body as ContextualView;
        return (
          <GraphView
            view={view}
            objectId={extra?.objectId ?? ""}
            paneId={extra?.paneId}
            onClose={extra?.onClose}
          />
        );
      },
    });

    // `table` — delegates to a simple table renderer.
    // Props accepted: `{ columns: string[], rows: Record<string, unknown>[] }`.
    this.register("table", {
      label: "Table",
      render: (body) => <TableRenderer body={body} />,
    });

    // `tree` — delegates to a simple tree renderer.
    // Props accepted: `{ nodes: TreeNode[] }`.
    this.register("tree", {
      label: "Tree",
      render: (body) => <TreeRenderer body={body} />,
    });

    // `code` — renders a code block with syntax highlighting hint.
    // Props accepted: `{ code: string, language?: string }`.
    this.register("code", {
      label: "Code",
      render: (body) => <CodeRenderer body={body} />,
    });

    // `json` — renders pretty-printed JSON. Safe fallback.
    this.register("json", {
      label: "JSON",
      render: (body) => <JsonRenderer body={body} />,
    });

    // `markdown` — renders markdown string as HTML (basic conversion).
    // Props accepted: `{ source: string }`.
    this.register("markdown", {
      label: "Markdown",
      render: (body) => <MarkdownRenderer body={body} />,
    });

    // `vega_lite` — placeholder for Vega-Lite charts.
    // Props accepted: `{ spec: unknown }`.
    // TODO: wire to vega-lite component in Phase 4.
    this.register("vega_lite", {
      label: "Vega-Lite (placeholder)",
      render: (body) => (
        <div
          data-testid="renderer-vega-lite-placeholder"
          className="rounded-md p-4"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-muted)",
            border: "1px dashed var(--color-border)",
          }}
        >
          <p className="text-xs">Vega-Lite renderer — spec in body</p>
          <pre className="mt-2 overflow-x-auto font-mono text-xs">
            {JSON.stringify(body, null, 2).slice(0, 500)}
          </pre>
        </div>
      ),
    });

    // `composite` — renders an ordered list of renderer ids with their data.
    // Props accepted: `{ parts: Array<{ renderer: RendererKind, body: unknown }> }`.
    this.register("composite", {
      label: "Composite",
      render: (body) => <CompositeRenderer body={body} registry={this} />,
    });
  }

  #getJsonRenderer(): RendererEntry {
    // This will always exist because #registerBuiltin runs in the constructor.
    return (
      this.#map.get("json") ?? {
        label: "JSON (fallback)",
        render: (body) => <JsonRenderer body={body} />,
      }
    );
  }
}

// ============================================================================
// Singleton instance
// ============================================================================

/**
 * The global renderer registry singleton. Import this in components that
 * need to render ViewSpec-driven content.
 *
 * Usage:
 * ```ts
 * import { rendererRegistry } from "../components/rendererRegistry";
 *
 * const entry = rendererRegistry.getOrJson(rendererKind);
 * return <>{entry.render(props.body)}</>;
 * ```
 */
export const rendererRegistry = new RendererRegistry();

// ============================================================================
// Built-in renderer components
// ============================================================================

// The `graph` entry is now the canonical GraphView (E1.5). The remaining
// entries (table, tree, code, json, markdown, vega_lite, composite) are
// built-in renderers for their respective RendererKind strings.

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function TableRenderer({ body }: { body: unknown }) {
  const b = body as { columns?: string[]; rows?: Record<string, unknown>[] } | null;
  if (!b?.columns?.length || !Array.isArray(b.rows)) {
    return <JsonRenderer body={body} />;
  }
  return (
    <div data-testid="renderer-table" className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr>
            {b.columns.map((col) => (
              <th
                key={col}
                className="border px-3 py-1 text-left font-medium"
                style={{ borderColor: "var(--color-border)" }}
              >
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {b.rows.map((row, i) => (
            <tr key={i}>
              {b.columns!.map((col) => (
                <td
                  key={col}
                  className="border px-3 py-1 font-mono text-xs"
                  style={{ borderColor: "var(--color-border)" }}
                >
                  {String(row[col] ?? "")}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function TreeRenderer({ body }: { body: unknown }) {
  const b = body as { label?: string; children?: unknown[] } | null;
  if (!b?.label) {
    return <JsonRenderer body={body} />;
  }
  return (
    <div data-testid="renderer-tree" className="text-sm">
      <TreeNode label={b.label ?? ""} children={b.children} registry={null} />
    </div>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function TreeNode({
  label,
  children,
  registry,
}: {
  label: string;
  children: unknown[] | undefined;
  registry: RendererRegistry | null;
}) {
  const [open, setOpen] = React.useState(false);
  const hasChildren = Array.isArray(children) && children.length > 0;
  return (
    <div className="flex flex-col">
      <button
        type="button"
        className="flex items-center gap-1 cursor-pointer text-left text-sm"
        onClick={() => setOpen((o) => !o)}
        data-testid="renderer-tree-node"
      >
        <span style={{ color: "var(--color-text-muted)" }}>
          {hasChildren ? (open ? "▼" : "▶") : "·"}
        </span>
        <span style={{ color: "var(--color-text-primary)" }}>{label}</span>
      </button>
      {open &&
        hasChildren &&
        (children as Array<{ label?: string; children?: unknown[] }>).map(
          (child, i) => (
            <div key={i} className="ml-4">
              <TreeNode
                label={child.label ?? "(unknown)"}
                children={child.children}
                registry={registry}
              />
            </div>
          ),
        )}
    </div>
  );
}

// eslint-disable-next-line @typescript-eslint/no-require-imports
const React = require("react") as typeof import("react");

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function CodeRenderer({ body }: { body: unknown }) {
  const b = body as { code?: string; language?: string; file?: string } | null;
  const code = b?.code ?? JSON.stringify(body, null, 2);
  const language = b?.language ?? detectLanguage(b?.file ?? "");
  return (
    <div data-testid="renderer-code">
      <pre
        tabIndex={0}
        className="overflow-x-auto rounded-sm p-3 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code>{highlightCode(code, language)}</code>
      </pre>
    </div>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function JsonRenderer({ body }: { body: unknown }) {
  return (
    <div data-testid="renderer-json">
      <pre
        tabIndex={0}
        className="overflow-x-auto rounded-sm p-3 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code>{JSON.stringify(body, null, 2)}</code>
      </pre>
    </div>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function MarkdownRenderer({ body }: { body: unknown }) {
  const b = body as { source?: string } | null;
  const source = b?.source ?? "";
  return (
    <div
      data-testid="renderer-markdown"
      className="prose prose-sm max-w-none"
      // Basic markdown-to-HTML: just render as preformatted for now.
      // Proper markdown rendering (e.g., react-markdown) will be wired in Phase 4.
    >
      <pre
        className="whitespace-pre-wrap text-sm"
        style={{ color: "var(--color-text-primary)" }}
      >
        {source}
      </pre>
    </div>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
function CompositeRenderer({
  body,
  registry,
}: {
  body: unknown;
  registry: RendererRegistry;
}) {
  const b = body as {
    parts?: Array<{ renderer?: string; body?: unknown }>;
  } | null;
  const parts = b?.parts;
  if (!Array.isArray(parts) || parts.length === 0) {
    return (
      <div
        data-testid="renderer-composite-empty"
        style={{ color: "var(--color-text-muted)" }}
      >
        No parts in composite.
      </div>
    );
  }
  return (
    <div
      data-testid="renderer-composite"
      className="flex flex-col gap-4"
    >
      {parts.map((part, i) => (
        <div key={i} data-testid={`renderer-composite-part-${i}`}>
          <div className="mb-1 text-xs font-medium uppercase tracking-wide"
            style={{ color: "var(--color-text-secondary)" }}>
            {part.renderer ?? "unknown"}
          </div>
          <div>
            {registry.render(part.renderer ?? "json", part.body ?? null)}
          </div>
        </div>
      ))}
    </div>
  );
}

// ============================================================================
// Type exports for consumers
// ============================================================================

export type { RendererKind } from "../api/schemas";
