/**
 * `Shell` — the responsive layout for the Explorer.
 *
 * Breakpoint strategy (from the design):
 * - Ultrawide (≥ 1440px): Miller Columns | Object Inspector | Lens
 *   Panel | InteractiveGraph — 4-column grid. The 4th column is
 *   `React.lazy` to keep cytoscape + elkjs out of the initial
 *   bundle.
 * - Desktop (1200 – 1439px): Miller Columns | Object Inspector |
 *   Lens Panel — 3-column grid.
 * - Tablet (900 – 1199px): Miller Columns | Object Inspector; Lens
 *   Panel becomes a toggleable overlay.
 * - Small (< 900px): a single drill-down column (ObjectInspector takes
 *   the whole pane; the user navigates with the Spotter / keyboard).
 *
 * The Shell owns the responsive decision and renders the panels
 * accordingly. Panels themselves do not know about breakpoints.
 */
import { Suspense, lazy, useEffect, useState, type ReactNode } from "react";

import { useAppDispatch, useAppState } from "../state/context";
import { MillerColumns } from "./MillerColumns/MillerColumns";
import { ObjectInspector } from "./ObjectInspector";
import { LensPanel } from "./LensPanel";
import { HealthProbe } from "./HealthProbe";
import { ErrorBoundary } from "./ErrorBoundary";
import { SketchLink } from "./SkipLink";
import { ScanBar } from "./ScanBar";
import { Spotter } from "./Spotter";
import { NavigationModeToggle } from "./Settings/NavigationModeToggle";
import { ShareExplorationButton } from "./ShareExplorationButton";
import { PaneStackView } from "./PaneStackView";
import { detectViewport, type ShellViewport } from "./viewport";
import { useRestoreExploration } from "../hooks/useRestoreExploration";
import { useSubgraph } from "../hooks/useSubgraph";
import { ContextualPanel } from "./ContextualPanel";

// `React.lazy` keeps the cytoscape + elkjs chunk out of the
// initial bundle — important for the 3-column desktop and tablet
// layouts that never render this column.
const InteractiveGraph = lazy(() =>
  import("./InteractiveGraph").then((m) => ({ default: m.InteractiveGraph })),
);

const RationaleView = lazy(() =>
  import("./RationaleView").then((m) => ({ default: m.RationaleView })),
);

function InteractiveGraphPanel({ rootId }: { rootId: string | null }) {
  const { activeLensId } = useAppState();
  const { data } = useSubgraph(rootId);

  // When the rationale lens is active, render RationaleView instead
  // of the default InteractiveGraph. The RationaleView wraps the
  // same InteractiveGraph but fetches from the rationale endpoint
  // and applies corroboration styles dynamically.
  if (activeLensId === "rationale" && rootId) {
    return (
      <RationaleView
        focusId={rootId}
        onSelectObject={() => {
          // Selection is read-only in this column for now.
        }}
      />
    );
  }

  return (
    <InteractiveGraph
      root={rootId ?? "—"}
      data={data}
      selectedId={rootId}
      onSelectObject={() => {
        // Selection is read-only in this column for now — selecting
        // a node just highlights it; navigation still happens via
        // Miller Columns / Spotter. The wiring is intentionally
        // open for the next iteration.
      }}
    />
  );
}

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
  // window width and listen for resize events.
  const [viewport, setViewport] = useState<ShellViewport>("desktop");
  // Lens overlay toggle (tablet mode only).
  const [lensOpen, setLensOpen] = useState(false);
  const dispatch = useAppDispatch();
  const appState = useAppState();

  // Restore exploration from ?exploration=<id> on mount (ADR-016 Fase 4).
  useRestoreExploration();

  // Sync the viewport state to the actual window size and re-evaluate
  // on resize. SSR-safe — `window` is undefined on the server and we
  // bail out before touching it. When a `viewportOverride` is supplied
  // (tests + Playwright), the resize listener is skipped entirely so
  // the controlled value always wins.
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (viewportOverride !== undefined) return;

    const update = () => {
      setViewport(detectViewport(window.innerWidth));
    };
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, [viewportOverride]);

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
          <ScanBar />
        </div>
        <div className="flex items-center gap-2">
          <NavigationModeToggle />
          <ShareExplorationButton />
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
        {renderPanels(
          activeViewport,
          lensOpen,
          () => setLensOpen(false),
          appState.activeObjectId,
          appState.navigation.mode,
        )}
      </main>
      <Spotter />
    </div>
  );
}

/**
 * Render the panel layout for the current viewport.
 *
 * - Ultrawide: 4-column CSS grid (MillerColumns | ObjectInspector |
 *   LensPanel | InteractiveGraph). The 4th column is wrapped in
 *   `<Suspense>` so the cytoscape chunk loads asynchronously.
 * - Desktop: 3-column CSS grid, no overlay.
 * - Tablet: 2-column grid, lens panel as a positioned overlay.
 * - Small: single column, ObjectInspector takes the full width.
 *
 * When navigation mode is `pane-stack` and viewport is NOT small,
 * the central column renders a `PaneStackView` instead of a single
 * `ObjectInspector`.
 */
function renderPanels(
  viewport: ShellViewport,
  lensOpen: boolean,
  onLensClose: () => void,
  activeObjectId: string | null,
  navigationMode: string,
): ReactNode {
  // Small viewport always uses column mode — pane-stack would overflow.
  const effectiveMode = viewport === "small" ? "column" : navigationMode;
  const inspector =
    effectiveMode === "pane-stack" ? (
      <ErrorBoundary label="PaneStackView">
        <PaneStackView />
      </ErrorBoundary>
    ) : (
      <ErrorBoundary label="ObjectInspector">
        <ObjectInspector />
      </ErrorBoundary>
    );

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
        {inspector}
        {lensOpen && (
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Lens panel"
            data-testid="lens-overlay"
            onKeyDown={(e) => {
              if (e.key === "Escape") onLensClose();
            }}
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

  if (viewport === "ultrawide") {
    return (
      <div
        className="grid h-full"
        style={{
          gridTemplateColumns:
            "minmax(0, 1fr) minmax(0, 1.4fr) minmax(0, 1fr) minmax(0, 1.4fr) minmax(0, 1.4fr)",
        }}
      >
        <ErrorBoundary label="MillerColumns">
          <MillerColumns />
        </ErrorBoundary>
        {inspector}
        <ErrorBoundary label="LensPanel">
          <LensPanel />
        </ErrorBoundary>
        <ErrorBoundary label="InteractiveGraph">
          <Suspense
            fallback={
              <div
                data-testid="interactive-graph-loading"
                style={{
                  height: "100%",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "var(--color-text-muted)",
                  fontSize: 12,
                }}
              >
                Loading graph…
              </div>
            }
          >
            <InteractiveGraphPanel rootId={activeObjectId} />
          </Suspense>
        </ErrorBoundary>
        <ErrorBoundary label="ContextualPanel">
          <ContextualPanel focusId={activeObjectId} />
        </ErrorBoundary>
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
      {inspector}
      <ErrorBoundary label="LensPanel">
        <LensPanel />
      </ErrorBoundary>
    </div>
  );
}
