/**
 * PaneStackView — horizontal carousel of side-by-side inspector panes.
 *
 * Renders one `PaneInspector` per entry in `state.navigation.panes`.
 * The active pane is highlighted; clicking a tab makes it active.
 * Closing a pane dispatches CLOSE_PANE and moves focus to a neighbour.
 *
 * CSS: a horizontal scroll container with snap points. Each pane is
 * `min-w-[320px]` and snaps to the viewport edge. The active pane
 * scrolls into view automatically via `scrollIntoView()`.
 *
 * GtPager-inspired: multiple objects inspected in parallel.
 */
import { useCallback, useEffect, useRef } from "react";
import { useApp, useAppDispatch } from "../state/context";
import { PaneInspector } from "./ObjectInspector/PaneInspector";

export function PaneStackView() {
  const { state } = useApp();
  const dispatch = useAppDispatch();
  const { panes, activePaneId } = state.navigation;
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll the active pane into view
  useEffect(() => {
    if (!activePaneId || !containerRef.current) return;
    const el = containerRef.current.querySelector(
      `[data-pane-id="${activePaneId}"]`,
    ) as HTMLElement | null;
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
    }
  }, [activePaneId, panes.length]);

  const handleClose = useCallback(
    (paneId: string) => {
      dispatch({ type: "CLOSE_PANE", payload: { paneId } });
    },
    [dispatch],
  );

  const handleActivate = useCallback(
    (paneId: string) => {
      dispatch({ type: "ACTIVATE_PANE", payload: { paneId } });
    },
    [dispatch],
  );

  const handleScroll = useCallback(
    (paneId: string, scrollY: number) => {
      dispatch({ type: "SET_PANE_SCROLL", payload: { paneId, scrollY } });
    },
    [dispatch],
  );

  if (panes.length === 0) {
    return (
      <div
        data-testid="pane-stack-empty"
        className="flex h-full items-center justify-center p-6 text-center text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <div>
          <p
            className="font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            No panes open
          </p>
          <p className="mt-1 text-xs">
            Select an object from the Miller Columns or Spotter to open a pane.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="pane-stack-view" className="flex h-full flex-col">
      {/* Tab strip */}
      <div
        className="flex items-center gap-0 overflow-x-auto px-1 py-1"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          borderBottom: "1px solid var(--color-border)",
        }}
        role="tablist"
        aria-label="Open panes"
      >
        {panes.map((pane) => (
          <button
            key={pane.id}
            role="tab"
            aria-selected={pane.id === activePaneId}
            data-pane-tab={pane.id}
            data-testid={`pane-tab-${pane.id}`}
            className={`truncate rounded-t-md px-3 py-1.5 text-xs font-medium transition-colors ${
              pane.id === activePaneId
                ? "border-b-2"
                : "hover:bg-black/5"
            }`}
            style={{
              maxWidth: "160px",
              borderBottomColor:
                pane.id === activePaneId
                  ? "var(--color-primary)"
                  : "transparent",
              color:
                pane.id === activePaneId
                  ? "var(--color-text-primary)"
                  : "var(--color-text-muted)",
              backgroundColor:
                pane.id === activePaneId
                  ? "var(--color-surface)"
                  : "transparent",
            }}
            onClick={() => handleActivate(pane.id)}
            title={pane.objectId}
          >
            {pane.objectId.split(":").pop()?.split("/").pop()?.slice(0, 30) ??
              pane.objectId}
          </button>
        ))}
      </div>

      {/* Pane carousel */}
      <div
        ref={containerRef}
        className="flex flex-1 overflow-x-auto snap-x snap-mandatory"
        style={{ scrollBehavior: "smooth" }}
      >
        {panes.map((pane) => {
          const isActive = pane.id === activePaneId;
          return (
            <div
              key={pane.id}
              data-pane-id={pane.id}
              data-testid={`pane-${pane.id}`}
              className={`flex-shrink-0 snap-start ${
                isActive ? "" : "opacity-50"
              }`}
              style={{
                width: "max(320px, calc(100% / max(1, panes.length, 3)))",
                minWidth: "320px",
                maxWidth: "100%",
                borderRight:
                  pane !== panes[panes.length - 1]
                    ? "1px solid var(--color-border)"
                    : "none",
              }}
            >
              <PaneInspector
                objectId={pane.objectId}
                viewId={pane.activeViewId}
                lensId={pane.activeLensId}
                activeView={pane.activeView}
                onClose={() => handleClose(pane.id)}
                onScroll={(scrollY) => handleScroll(pane.id, scrollY)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
