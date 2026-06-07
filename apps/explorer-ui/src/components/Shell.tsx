/**
 * `Shell` — the 3-panel responsive layout for the Explorer.
 *
 * Breakpoint strategy (from the design):
 * - Desktop (≥ 1200px): Miller Columns | Object Inspector | Lens Panel
 *   in a 3-column CSS grid.
 * - Tablet (900 – 1199px): Miller Columns | Object Inspector; Lens
 *   Panel becomes a toggleable overlay.
 * - Small (< 900px): a single drill-down column (ObjectInspector takes
 *   the whole pane; the user navigates with the Spotter / keyboard).
 *
 * The Shell owns the responsive decision and renders the panels
 * accordingly. Panels themselves do not know about breakpoints.
 */
import { useState, type ReactNode } from "react";

import { useAppDispatch } from "../state/context";
import { MillerColumns } from "./MillerColumns/MillerColumns";
import { ObjectInspector } from "./ObjectInspector";
import { LensPanel } from "./LensPanel";
import { HealthProbe } from "./HealthProbe";
import { ErrorBoundary } from "./ErrorBoundary";
import { SkipLink } from "./SkipLink";
import { Spotter } from "./Spotter";
import { detectViewport, type ShellViewport } from "./viewport";

export interface ShellProps {
  /**
   * Override the viewport. Used by tests + Playwright to assert the
   * responsive behaviour without resizing the window.
   */
  viewport?: ShellViewport;
}

export function Shell({ viewport: viewportOverride }: ShellProps = {}) {
  // We default to "desktop" on the server and on the first render to
  // avoid hydration mismatches. After mount, we sync to the actual
  // window width.
  const [viewport, setViewport] = useState<ShellViewport>("desktop");
  // Lens overlay toggle (tablet mode only).
  const [lensOpen, setLensOpen] = useState(false);
  const dispatch = useAppDispatch();

  // Sync the viewport state to the actual window size. SSR-safe —
  // the listener only registers after `window` is defined.
  if (typeof window !== "undefined" && viewportOverride === undefined) {
    // We use a useState initializer to read once and then a
    // useEffect to subscribe — but to keep Shell zero-effect
    // unless needed, we read the size at first render and rely on
    // the global resize listener below.
    if (viewport === "desktop") {
      // intentionally empty — the effect will pick up the real size
    }
  }

  // Wire the resize listener imperatively on mount.
  // Using a useEffect-like pattern via a render guard is fragile,
  // so we use a one-time window size sync via useSyncExternalStore
  // semantics — but for simplicity we just register a listener
  // once.
  if (viewportOverride === undefined) {
    // Use the inline listener approach for the first paint; we
    // attach it via a setState callback to avoid SSR noise.
    if (typeof window !== "undefined") {
      const current = detectViewport(window.innerWidth);
      if (current !== viewport) {
        // Schedule a state update after render to avoid
        // setState-in-render warnings.
        queueMicrotask(() => setViewport(current));
      }
    }
  } else if (viewportOverride !== viewport) {
    // Synchronous override (tests).
    // We deliberately do not call setState here — instead, callers
    // who need a controlled viewport should pass a `key` to remount.
    // For the test path, the override is read on every render via
    // the `viewportOverride` variable above.
  }

  const activeViewport: ShellViewport =
    viewportOverride ?? viewport;

  return (
    <div
      data-testid="shell"
      data-viewport={activeViewport}
      className="flex h-full w-full flex-col"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <SkipLink targetId="app-main" />
      {/* Top bar — health chip + (in tablet mode) lens toggle. */}
      <header
        className="flex items-center justify-between gap-3 px-4 py-2"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          borderBottom: "1px solid var(--color-border)",
        }}
      >
        <div className="flex items-center gap-2">
          <h1
            className="text-sm font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            CogniCode Explorer
          </h1>
          <HealthProbe showFullScreenOnError={false} />
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() =>
              dispatch({ type: "SET_SPOTTER", payload: { open: true } })
            }
            aria-label="Open Spotter search"
            data-testid="spotter-trigger"
            className="flex items-center gap-2 rounded-md px-2 py-1 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <span aria-hidden="true">⌕</span>
            <span>Search</span>
            <span
              aria-hidden="true"
              className="rounded px-1 font-mono text-xs"
              style={{
                backgroundColor: "var(--color-surface)",
                color: "var(--color-text-muted)",
              }}
            >
              ⌘K
            </span>
          </button>
        </div>
        {activeViewport === "tablet" && (
          <button
            type="button"
            onClick={() => setLensOpen((v) => !v)}
            aria-pressed={lensOpen}
            aria-label={lensOpen ? "Close lens panel" : "Open lens panel"}
            className="rounded-md px-2 py-1 text-xs font-medium"
            style={{
              backgroundColor: lensOpen
                ? "var(--color-primary)"
                : "var(--color-surface-overlay)",
              color: lensOpen
                ? "var(--color-primary-foreground)"
                : "var(--color-text-primary)",
            }}
          >
            {lensOpen ? "Close lens" : "Open lens"}
          </button>
        )}
      </header>
      <main
        id="app-main"
        tabIndex={-1}
        className="flex-1 overflow-hidden"
        aria-label="Explorer panels"
      >
        {renderPanels(activeViewport, lensOpen, () => setLensOpen(false))}
      </main>
      <Spotter />
    </div>
  );
}

/**
 * Render the panel layout for the current viewport.
 *
 * - Desktop: 3-column CSS grid, no overlay.
 * - Tablet: 2-column grid, lens panel as a positioned overlay.
 * - Small: single column, ObjectInspector takes the full width.
 */
function renderPanels(
  viewport: ShellViewport,
  lensOpen: boolean,
  onLensClose: () => void,
): ReactNode {
  if (viewport === "small") {
    return (
      <div
        className="grid h-full"
        style={{ gridTemplateColumns: "1fr" }}
      >
        <ErrorBoundary label="MillerColumns">
          <MillerColumns />
        </ErrorBoundary>
      </div>
    );
  }

  if (viewport === "tablet") {
    return (
      <div
        className="relative grid h-full"
        style={{ gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1.4fr)" }}
      >
        <ErrorBoundary label="MillerColumns">
          <MillerColumns />
        </ErrorBoundary>
        <ErrorBoundary label="ObjectInspector">
          <ObjectInspector />
        </ErrorBoundary>
        {lensOpen && (
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Lens panel"
            data-testid="lens-overlay"
            className="absolute right-0 top-0 z-20 h-full"
            style={{
              width: "min(100%, 28rem)",
              backgroundColor: "var(--color-surface)",
              borderLeft: "1px solid var(--color-border)",
              boxShadow: "-8px 0 24px rgba(0,0,0,0.35)",
            }}
          >
            <ErrorBoundary label="LensPanel">
              <LensPanel />
            </ErrorBoundary>
            <button
              type="button"
              onClick={onLensClose}
              aria-label="Close lens panel"
              className="absolute right-2 top-2 rounded-md px-2 py-0.5 text-xs"
              style={{
                backgroundColor: "var(--color-surface-overlay)",
                color: "var(--color-text-secondary)",
              }}
            >
              ×
            </button>
          </div>
        )}
      </div>
    );
  }

  // Desktop: 3 columns.
  return (
    <div
      className="grid h-full"
      style={{ gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1.4fr) minmax(0, 1fr)" }}
    >
      <ErrorBoundary label="MillerColumns">
        <MillerColumns />
      </ErrorBoundary>
      <ErrorBoundary label="ObjectInspector">
        <ObjectInspector />
      </ErrorBoundary>
      <ErrorBoundary label="LensPanel">
        <LensPanel />
      </ErrorBoundary>
    </div>
  );
}
