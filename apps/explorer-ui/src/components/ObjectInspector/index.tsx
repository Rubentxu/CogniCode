/**
 * `ObjectInspector` — the centre panel of the Explorer.
 *
 * Composition:
 *   <ObjectInspector>
 *     <ViewTabs />            ← top strip (tabs)
 *     <ViewPanel />           ← content area
 *       <Blocks view=… />     ← all 27 typed block renderers
 *     </ViewPanel>
 *   </ObjectInspector>
 *
 * The container wires `useObject` (summary) and `useViews` (the
 * contextual view) and threads the active view id through
 * `ViewTabs` + `Blocks`. Selecting a tab dispatches
 * `SET_ACTIVE_VIEW` so the reducer caches the latest view for
 * instant re-render after navigation.
 */
import { useEffect } from "react";

import { useApp, useAppDispatch } from "../../state/context";
import { useObject } from "../../hooks/useObject";
import { useAvailableViews, useViews } from "../../hooks/useViews";
import { LoadingTier } from "../LoadingTier";
import { ViewTabs } from "./ViewTabs";
import { Blocks } from "./ViewBlock";

// Public surface — `import { ObjectInspector, ViewBlock } from
// "./components/ObjectInspector"` resolves here.
export { ViewTabs } from "./ViewTabs";
export type { ViewTabsProps } from "./ViewTabs";
export { ViewBlock, Blocks } from "./ViewBlock";
export type { ViewBlockProps, BlocksProps } from "./ViewBlock";

export function ObjectInspector() {
  const { state } = useApp();
  const dispatch = useAppDispatch();
  const { activeObjectId, activeViewId, activeView } = state;

  // Object summary — used to enumerate the available views.
  const {
    data: object,
    isLoading: isObjectLoading,
    isValidating: isObjectValidating,
    error: objectError,
  } = useObject(activeObjectId);

  // View descriptors for the tab strip.
  const { data: views } = useAvailableViews(activeObjectId);

  // The active contextual view. Falls back to the cached
  // `state.activeView` so the UI stays responsive while SWR
  // revalidates in the background.
  const {
    data: view,
    isLoading: isViewLoading,
    isValidating: isViewValidating,
    error: viewError,
  } = useViews(activeObjectId, activeViewId);

  // When a new view resolves, cache it in the reducer so the
  // next render (e.g., back-navigation) is instant.
  useEffect(() => {
    if (view) {
      dispatch({ type: "SET_ACTIVE_VIEW", payload: view });
    }
  }, [view, dispatch]);

  // If the user navigates to a new object and the current
  // `activeViewId` is not in the new object's `available_views`,
  // fall back to the first available view. This is the only
  // automatic tab-change the container performs.
  useEffect(() => {
    if (!views || views.length === 0) return;
    if (activeViewId && views.some((v) => v.id === activeViewId)) return;
    const firstId = views[0]?.id;
    if (firstId) {
      dispatch({
        type: "SELECT_OBJECT",
        payload: { objectId: activeObjectId!, viewId: firstId },
      });
    }
  }, [views, activeViewId, activeObjectId, dispatch]);

  if (!activeObjectId) {
    return (
      <div
        data-testid="object-inspector-empty"
        className="flex h-full items-center justify-center p-6 text-center text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <div>
          <p
            className="font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            No object selected
          </p>
          <p className="mt-1 text-xs">
            Drill into the Miller Columns or open the Spotter.
          </p>
        </div>
      </div>
    );
  }

  // Show the cached view (if any) until the new one resolves —
  // this keeps the inspector feeling instant.
  const display = view ?? activeView;
  const blockCount = display?.blocks.length ?? 0;
  const showLoadingShell = !display && (isObjectLoading || isViewLoading);
  const error = objectError ?? viewError ?? null;

  return (
    <LoadingTier
      data={display ?? object}
      isLoading={showLoadingShell}
      isValidating={isObjectValidating || isViewValidating}
      error={error}
      label="Object inspector"
    >
      <div
        data-testid="object-inspector"
        className="flex h-full flex-col"
        style={{ backgroundColor: "var(--color-surface)" }}
      >
        <header
          className="flex items-center justify-between gap-2 px-4 py-2"
          style={{ borderBottom: "1px solid var(--color-border)" }}
        >
          <h2
            className="truncate text-sm font-semibold"
            style={{ color: "var(--color-text-primary)" }}
            title={display?.title ?? object?.label ?? ""}
          >
            {display?.title ?? object?.label ?? "(loading)"}
          </h2>
          <span
            className="rounded-full px-2 py-0.5 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-muted)",
            }}
          >
            {blockCount} {blockCount === 1 ? "block" : "blocks"}
          </span>
        </header>
        {views && views.length > 0 && (
          <ViewTabs
            views={views}
            activeViewId={activeViewId}
            isLoading={isViewLoading}
            onChange={(viewId) => {
              if (!activeObjectId) return;
              dispatch({
                type: "SELECT_OBJECT",
                payload: { objectId: activeObjectId, viewId },
              });
            }}
          />
        )}
        <div
          role="tabpanel"
          id={activeViewId ? `view-tab-panel-${activeViewId}` : undefined}
          aria-labelledby={
            activeViewId ? `view-tab-${activeViewId}` : undefined
          }
          tabIndex={0}
          data-testid="object-inspector-body"
          className="flex-1 overflow-y-auto p-4 text-sm"
          style={{ color: "var(--color-text-secondary)" }}
        >
          {display ? (
            <Blocks
              view={display}
              onSelectObject={(objectId) =>
                dispatch({
                  type: "SELECT_OBJECT",
                  payload: { objectId, viewId: "overview" },
                })
              }
            />
          ) : (
            <p
              className="text-sm"
              style={{ color: "var(--color-text-muted)" }}
            >
              No view loaded.
            </p>
          )}
        </div>
      </div>
    </LoadingTier>
  );
}
