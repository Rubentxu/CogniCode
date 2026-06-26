/**
 * `ContextualPanel` — orchestrator for the contextual view (Phase 1
 * of Contextual Views). Wires the `useContextualGraph` SWR hook to
 * the four subcomponents:
 * - `FocusCard`           — always visible
 * - `ParentBreadcrumb`    — visible when `parent` is non-null
 * - `ChildrenList`        — visible when `children` is non-null
 * - `NeighborMinigraph`   — always visible when `sameLevel` has data
 * - `TruncationBanner`    — visible when `truncated` is true
 *
 * Loading + error states are rendered as small inline messages so
 * the panel can be embedded in any column without disturbing the
 * surrounding layout.
 */
import { useContextualGraph } from "../../hooks/useContextualGraph";
import type { ContextualOptions } from "../../api/client";
import { ChildrenList } from "./ChildrenList";
import { FocusCard } from "./FocusCard";
import { NeighborMinigraph } from "./NeighborMinigraph";
import { ParentBreadcrumb } from "./ParentBreadcrumb";
import { TruncationBanner } from "./TruncationBanner";
import styles from "./ContextualPanel.module.css";

export interface ContextualPanelProps {
  /** MVP id of the focus symbol. `null` renders an empty state. */
  focusId: string | null;
  /** Optional fetch opts (level / depth / maxNodes). */
  opts?: ContextualOptions;
  /** Optional CSS class passthrough on the root element. */
  className?: string;
}

export function ContextualPanel({
  focusId,
  opts,
  className,
}: ContextualPanelProps) {
  const { data, error, isLoading } = useContextualGraph(focusId, opts);

  if (!focusId) {
    return (
      <div
        data-testid="contextual-panel-empty"
        className={className}
        style={{ padding: 12, color: "var(--color-text-muted)", fontSize: 12 }}
      >
        Select a symbol to see its context.
      </div>
    );
  }

  if (isLoading) {
    return (
      <div
        data-testid="contextual-panel-loading"
        className={className}
        style={{ padding: 12, color: "var(--color-text-muted)", fontSize: 12 }}
      >
        Loading contextual view…
      </div>
    );
  }

  if (error) {
    const status = (error as { status?: number } | undefined)?.status;
    return (
      <div
        data-testid="contextual-panel-error"
        role="alert"
        className={className}
        style={{ padding: 12, color: "#b91c1c", fontSize: 12 }}
      >
        {status === 404
          ? `Symbol not found: ${focusId}`
          : `Failed to load contextual view: ${(error as Error).message}`}
      </div>
    );
  }

  if (!data) {
    return null;
  }

  const onFocus = (id: string) => {
    // The ContextualPanel only renders one focus at a time. A click
    // on any section (parent, child, neighbor) bubbles to the
    // surrounding app via the SWR cache key — the parent
    // component should re-render ContextualPanel with the new
    // focusId. The orchestrator exposes this callback for the
    // subcomponents but does not own navigation state itself.
    if (typeof window !== "undefined") {
      // Update the URL hash so the focus is shareable. The Shell
      // listens for hashchange and updates `activeObjectId`.
      const next = `#${id}`;
      if (window.location.hash !== next) {
        window.location.hash = next;
      }
    }
  };

  return (
    <div
      data-testid="contextual-panel"
      className={`${styles.panel ?? ""} ${className ?? ""}`.trim()}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 10,
        height: "100%",
        overflowY: "auto",
      }}
    >
      {data.truncated && (
        <TruncationBanner reason={data.truncatedReason ?? "max_nodes_exceeded"} />
      )}
      <FocusCard focus={data.focusNode} />
      {data.parent && <ParentBreadcrumb parent={data.parent.node} onFocus={onFocus} />}
      {data.children && (
        <ChildrenList children={data.children.nodes} onFocus={onFocus} />
      )}
      {(data.sameLevel.nodes.length > 0 || data.sameLevel.edges.length > 0) && (
        <NeighborMinigraph
          focus={data.focusNode}
          nodes={data.sameLevel.nodes}
          edges={data.sameLevel.edges}
          onFocus={onFocus}
        />
      )}
    </div>
  );
}
