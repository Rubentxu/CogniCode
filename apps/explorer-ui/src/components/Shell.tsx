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
 * This component is the orchestrator — it composes ShellBootstrap (effects)
 * and ShellLayout (pure layout). The public API (props, data-testid, data-viewport)
 * is preserved unchanged.
 */
import { Suspense, lazy } from "react";

import { useAppDispatch, useAppState } from "../state/context";
import { ErrorBoundary } from "./ErrorBoundary";
import { Spotter } from "./Spotter";
import { PaneStackView } from "./PaneStackView";
import { ShellBootstrap } from "./ShellBootstrap";
import { ShellLayout } from "./ShellLayout";
import { useSubgraph } from "../hooks/useSubgraph";
import { GraphLanding } from "./GraphLanding";
import type { ShellViewport } from "./viewport";

// `React.lazy` keeps the cytoscape + elkjs chunk out of the
// initial bundle.
const InteractiveGraph = lazy(() =>
  import("./InteractiveGraph").then((m) => ({ default: m.InteractiveGraph })),
);

const RationaleView = lazy(() =>
  import("./RationaleView").then((m) => ({ default: m.RationaleView })),
);

export interface ShellProps {
  /**
   * Override the viewport. Used by tests + Playwright to assert the
   * responsive behaviour without resizing the window.
   */
  viewport?: ShellViewport;
}

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

const GRAPH_LOADING = (
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
);

export function Shell({ viewport: viewportOverride }: ShellProps = {}) {
  const dispatch = useAppDispatch();
  const appState = useAppState();
  const rootId = appState.activeObjectId;

  return (
    <ShellBootstrap>
      {({ workspace }) => (
        <>
          <ShellLayout
            viewport={viewportOverride}
            workspace={workspace}
            onSpotterOpen={() =>
              dispatch({ type: "SET_SPOTTER", payload: { open: true } })
            }
            secondaryContent={
              <ErrorBoundary label="PaneStackView">
                <PaneStackView />
              </ErrorBoundary>
            }
          >
            <ErrorBoundary label="InteractiveGraph">
              <Suspense fallback={GRAPH_LOADING}>
                {rootId === null && workspace ? (
                  <GraphLanding workspaceId={workspace.id} />
                ) : (
                  <InteractiveGraphPanel rootId={rootId} />
                )}
              </Suspense>
            </ErrorBoundary>
          </ShellLayout>
          <Spotter />
        </>
      )}
    </ShellBootstrap>
  );
}
