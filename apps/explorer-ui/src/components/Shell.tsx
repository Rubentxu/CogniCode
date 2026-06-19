/**
 * `Shell` — the responsive 2-zone layout for the Explorer.
 *
 * Layout architecture (post E3/ADR-039 column-nav removal):
 * - Desktop / Tablet / Ultrawide: 2-zone CSS grid
 *   `gridTemplateColumns: "minmax(0,1.4fr) minmax(0,1fr)"`
 *   Left zone: InteractiveGraph (primary). Right zone: PaneStackView (secondary).
 * - Small (< 900px): InteractiveGraph full-width; PaneStackView slides up
 *   as a bottom sheet (position:absolute, bottom:0, height:60vh, z-index:20).
 *
 * The graph is driven by `appState.activeObjectId`. `useSubgraph(rootId)`
 * provides the data. `useRestoreExploration()` handles ?exploration=<id>
 * restore on mount.
 */
import { Suspense, lazy, useEffect, useState } from "react";

import { useAppDispatch, useAppState } from "../state/context";
import { HealthProbe } from "./HealthProbe";
import { ErrorBoundary } from "./ErrorBoundary";
import { SkipLink } from "./SkipLink";
import { ScanBar } from "./ScanBar";
import { Spotter } from "./Spotter";
import { ShareExplorationButton } from "./ShareExplorationButton";
import { PaneStackView } from "./PaneStackView";
import { detectViewport, type ShellViewport } from "./viewport";
import { useRestoreExploration } from "../hooks/useRestoreExploration";
import { useSubgraph } from "../hooks/useSubgraph";

// `React.lazy` keeps the cytoscape + elkjs chunk out of the
// initial bundle.
const InteractiveGraph = lazy(() =>
  import("./InteractiveGraph").then((m) => ({ default: m.InteractiveGraph })),
);

const RationaleView = lazy(() =>
  import("./RationaleView").then((m) => ({ default: m.RationaleView })),
);

function InteractiveGraphPanel({ rootId }: { rootId: string | null }) {
  const { activeLensId } = useAppState();
  const { data } = useSubgraph(rootId);

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
        // a node just highlights it; navigation happens via PaneStackView.
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
  const [viewport, setViewport] = useState<ShellViewport>("desktop");
  const dispatch = useAppDispatch();
  const appState = useAppState();

  // Restore exploration from ?exploration=<id> on mount (ADR-016 Fase 4).
  useRestoreExploration();

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

  const isSmall = activeViewport === "small";
  const rootId = appState.activeObjectId;

  return (
    <div
      data-testid="shell"
      data-viewport={activeViewport}
      className="flex h-full w-full flex-col"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <SkipLink targetId="app-main" />
      {/* Top bar */}
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
      </header>
      <main
        id="app-main"
        tabIndex={-1}
        className="flex-1 overflow-hidden"
        aria-label="Explorer panels"
      >
        {isSmall ? (
          <div className="relative grid h-full" style={{ gridTemplateColumns: "1fr" }}>
            {/* Graph — full width on small viewport */}
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
                <InteractiveGraphPanel rootId={rootId} />
              </Suspense>
            </ErrorBoundary>
            {/* Bottom sheet — PaneStackView slides up from bottom */}
            <div
              data-testid="bottom-sheet"
              className="absolute left-0 right-0 top-1/2 z-20"
              style={{
                bottom: 0,
                height: "60vh",
                backgroundColor: "var(--color-surface)",
                borderTop: "1px solid var(--color-border)",
                boxShadow: "0 -8px 24px rgba(0,0,0,0.35)",
              }}
            >
              <ErrorBoundary label="PaneStackView">
                <PaneStackView />
              </ErrorBoundary>
            </div>
          </div>
        ) : (
          /* Desktop / Tablet / Ultrawide: 2-zone grid */
          <div
            className="grid h-full"
            style={{ gridTemplateColumns: "minmax(0,1.4fr) minmax(0,1fr)" }}
          >
            {/* Left — InteractiveGraph (primary) */}
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
                <InteractiveGraphPanel rootId={rootId} />
              </Suspense>
            </ErrorBoundary>
            {/* Right — PaneStackView (secondary) */}
            <ErrorBoundary label="PaneStackView">
              <PaneStackView />
            </ErrorBoundary>
          </div>
        )}
      </main>
      <Spotter />
    </div>
  );
}
