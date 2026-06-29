/**
 * PaneBreadcrumb — renders the causal breadcrumb below the pane header.
 *
 * Shows "From: <label> · Via: <view title>" when fromObjectId is set.
 * Clicking the "From" label dispatches SELECT_OBJECT to navigate to
 * that object (existing dedupe logic activates or pushes).
 * Also shows "n to add note" keyboard hint.
 *
 * Hidden entirely when fromObjectId is absent (legacy pane).
 *
 * NOTE: workspaceId is passed as a prop (not fetched internally) to ensure
 * correct resolution in cross-workspace drill-downs. Using useWorkspaceList()[0]
 * would silently resolve to the first workspace regardless of which workspace
 * the pane belongs to.
 */
import { useAppDispatch } from "../../state/context";
import { useObject } from "../../hooks/useObject";
import { useAvailableViews } from "../../hooks/useViews";

type PaneBreadcrumbProps = {
  fromObjectId: string;
  viaViewKind: string;
  /** Workspace ID for resolving available views — must be the workspace of the FROM object, not the active workspace. */
  workspaceId: string | null;
};

export function PaneBreadcrumb({ fromObjectId, viaViewKind, workspaceId }: PaneBreadcrumbProps) {
  const dispatch = useAppDispatch();

  const { data: fromObject } = useObject(fromObjectId);

  // Resolve view title for viaViewKind (a view id like "call_graph").
  const { data: availableViews } = useAvailableViews(fromObjectId, workspaceId, "default");
  const viewTitle = availableViews?.find((v) => v.id === viaViewKind)?.title ?? viaViewKind;

  const fromLabel = fromObject?.label ?? fromObjectId;

  function handleFromClick() {
    dispatch({
      type: "SELECT_OBJECT",
      payload: { objectId: fromObjectId, viewId: viaViewKind },
    });
  }

  return (
    <div
      data-testid="pane-breadcrumb"
      className="flex items-center gap-2 px-4 py-1 text-xs"
      style={{ color: "var(--color-text-muted)" }}
    >
      <span>
        From:{" "}
        <button
          type="button"
          onClick={handleFromClick}
          className="cursor-pointer underline hover:text-blue-500"
          title={`Navigate to ${fromLabel}`}
          data-testid="pane-breadcrumb-from"
        >
          {fromLabel}
        </button>
      </span>
      <span>·</span>
      <span>
        Via: <span data-testid="pane-breadcrumb-via">{viewTitle}</span>
      </span>
      <span className="ml-auto text-[10px]" style={{ color: "var(--color-text-muted)" }}>
        n to add note
      </span>
    </div>
  );
}
