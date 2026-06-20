/**
 * PaneInspector — a single Object Inspector instance parameterised by
 * objectId. Used by both column mode (via ObjectInspector reading state)
 * and pane-stack mode (one per pane, reading state.navigation).
 *
 * When `onClose` is provided, a close button appears (pane-stack only).
 */
import { useEffect, useState } from "react";
import { useApp, useAppDispatch } from "../../state/context";
import { useObject } from "../../hooks/useObject";
import { useAvailableViews, useViews } from "../../hooks/useViews";
import { useAsk } from "../../hooks/useAsk";
import { useWorkspaceList } from "../../hooks/useWorkspace";
import { LoadingTier } from "../LoadingTier";
import { detectViewport, type ShellViewport } from "../viewport";
import { ViewTabs } from "./ViewTabs";
import { SuggestionStrip } from "./SuggestionStrip";
import { Blocks } from "./ViewBlock";
import { ViewSpecWizard } from "./ViewSpecWizard";
import { multimodalLabelForObjectType } from "./multimodal";
import { GraphViewRenderer } from "../GraphView/GraphViewRenderer";

// Graph-shaped ViewKinds that route to GraphViewRenderer
function isGraphViewKind(kind: string | undefined): boolean {
  return (
    kind === "call_graph" ||
    kind === "dependency_graph" ||
    kind === "data_flow" ||
    kind === "impact_radius" ||
    kind === "seam_map"
  );
}

type PaneInspectorProps = {
  objectId: string;
  viewId: string | null;
  lensId: string | null;
  activeView: import("../../api/types").ContextualView | null;
  /**
   * Optional close callback — only shown when present (pane-stack).
   * Dispatches CLOSE_PANE with the pane's id.
   */
  onClose?: () => void;
  /** Pane-scroll sync (pane-stack only). */
  onScroll?: (scrollY: number) => void;
};

export function PaneInspector({
  objectId,
  viewId,
  lensId: _lensId,
  activeView: _activeView,
  onClose,
  onScroll,
}: PaneInspectorProps) {
  const dispatch = useAppDispatch();

  // Object summary
  const {
    data: object,
    isLoading: isObjectLoading,
    isValidating: isObjectValidating,
    error: objectError,
  } = useObject(objectId);

  // Workspace context
  const { data: workspaceList } = useWorkspaceList();
  const workspaceId = workspaceList?.[0]?.id ?? null;
  const wizardOwner = "default";

  // View descriptors
  const { data: views } = useAvailableViews(objectId, workspaceId, wizardOwner);

  // Active contextual view
  const {
    data: view,
    isLoading: isViewLoading,
    isValidating: isViewValidating,
    error: viewError,
  } = useViews(objectId, viewId);

  // Cache view in reducer
  useEffect(() => {
    if (view) {
      dispatch({ type: "SET_ACTIVE_VIEW", payload: view });
    }
  }, [view, dispatch]);

  // Graph status for suggestion strip
  const graphStatus = workspaceList?.[0]?.graph_status ?? null;

  const { dispatch: askDispatch } = useAsk({
    objectId,
    objectLabel: object?.label ?? null,
  });

  // Viewport
  const [viewport, setViewport] = useState<ShellViewport>(() =>
    typeof window === "undefined"
      ? "desktop"
      : detectViewport(window.innerWidth),
  );
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onResize = () => setViewport(detectViewport(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const [wizardOpen, setWizardOpen] = useState(false);

  // Fallback to first view if activeViewId is not in the list
  useEffect(() => {
    if (!views || views.length === 0) return;
    if (viewId && views.some((v) => v.id === viewId)) return;
    const firstId = views[0]?.id;
    if (firstId) {
      dispatch({
        type: "SELECT_OBJECT",
        payload: { objectId, viewId: firstId },
      });
    }
  }, [views, viewId, objectId, dispatch]);

  // Scroll sync
  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    onScroll?.(e.currentTarget.scrollTop);
  };

  const display = view ?? _activeView;
  const blockCount = display?.blocks.length ?? 0;
  const showLoadingShell = !display && (isObjectLoading || isViewLoading);
  const error = objectError ?? viewError ?? null;

  return (
    <>
      <LoadingTier
        data={display ?? object}
        isLoading={showLoadingShell}
        isValidating={isObjectValidating || isViewValidating}
        error={error}
        label={`Pane inspector: ${object?.label ?? objectId}`}
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
            <div className="flex min-w-0 items-center gap-2">
              <h2
                className="truncate text-sm font-semibold"
                style={{ color: "var(--color-text-primary)" }}
                title={display?.title ?? object?.label ?? ""}
              >
                {display?.title ?? object?.label ?? "(loading)"}
              </h2>
              {object && multimodalLabelForObjectType(object.object_type) && (
                <span
                  data-testid="multimodal-kind-badge"
                  className="rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase"
                  style={{
                    backgroundColor: "var(--color-surface-overlay)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  {multimodalLabelForObjectType(object.object_type)}
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              <span
                className="rounded-full px-2 py-0.5 text-xs"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-muted)",
                }}
              >
                {blockCount} {blockCount === 1 ? "block" : "blocks"}
              </span>
              {onClose && (
                <button
                  type="button"
                  aria-label="Close pane"
                  data-testid="pane-close"
                  onClick={onClose}
                  className="rounded-md px-1.5 py-0.5 text-xs hover:bg-red-100"
                  style={{
                    color: "var(--color-text-muted)",
                  }}
                >
                  ✕
                </button>
              )}
            </div>
          </header>
          {object && (
            <SuggestionStrip
              objectType={object.object_type}
              objectId={object.id}
              objectLabel={object.label}
              graphStatus={graphStatus}
              viewport={viewport}
              onDispatch={askDispatch}
            />
          )}
          {views && views.length > 0 && (
            <ViewTabs
              views={views}
              activeViewId={viewId}
              isLoading={isViewLoading}
              onChange={(vId) => {
                dispatch({
                  type: "SELECT_OBJECT",
                  payload: { objectId, viewId: vId },
                });
              }}
              objectId={object?.id}
              objectType={object?.object_type}
              objectLabel={object?.label}
              onOpenWizard={() => setWizardOpen(true)}
            />
          )}
          <div
            role="tabpanel"
            id={viewId ? `view-tab-panel-${viewId}` : undefined}
            aria-labelledby={viewId ? `view-tab-${viewId}` : undefined}
            tabIndex={0}
            data-testid="object-inspector-body"
            className="flex-1 overflow-y-auto p-4 text-sm"
            style={{ color: "var(--color-text-secondary)" }}
            onScroll={handleScroll}
          >
            {display ? (
              isGraphViewKind(display.view_kind) ? (
                <GraphViewRenderer
                  view={display}
                  objectId={objectId}
                  onClose={onClose}
                />
              ) : (
                <Blocks
                  view={display}
                  onSelectObject={(objId) =>
                    dispatch({
                      type: "SELECT_OBJECT",
                      payload: { objectId: objId, viewId: "overview" },
                    })
                  }
                />
              )
            ) : (
              <p className="text-sm" style={{ color: "var(--color-text-muted)" }}>
                No view loaded.
              </p>
            )}
          </div>
        </div>
      </LoadingTier>
      {wizardOpen && object && workspaceId && (
        <ViewSpecWizard
          isOpen={wizardOpen}
          onClose={() => setWizardOpen(false)}
          objectId={object.id}
          objectType={object.object_type}
          objectLabel={object.label}
          workspaceId={workspaceId}
          owner={wizardOwner}
          onSaved={() => {}}
        />
      )}
    </>
  );
}
