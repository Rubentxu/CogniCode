/**
 * PaneBreadcrumb — renders the causal breadcrumb below the pane header.
 *
 * Shows "From: <label> · Via: <view title>" when fromObjectId is set.
 * Clicking the "From" label dispatches SELECT_OBJECT to navigate to
 * that object (existing dedupe logic activates or pushes).
 * Also shows "n to add note" keyboard hint.
 *
 * Hidden entirely when fromObjectId is absent (legacy pane).
 */
import { useAppDispatch } from "../../state/context";
import { useObject } from "../../hooks/useObject";
import { useAvailableViews } from "../../hooks/useViews";
import { useWorkspaceList } from "../../hooks/useWorkspace";

type PaneBreadcrumbProps = {
  fromObjectId: string;
  viaViewKind: string;
};

export function PaneBreadcrumb({ fromObjectId, viaViewKind }: PaneBreadcrumbProps) {
  const dispatch = useAppDispatch();

  const { data: fromObject } = useObject(fromObjectId);
  const { data: workspaceList } = useWorkspaceList();
  const workspaceId = workspaceList?.[0]?.id ?? null;

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
