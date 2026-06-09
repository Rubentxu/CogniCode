/**
 * `ParentBreadcrumb` — second section of the `ContextualPanel`.
 * Renders the containing file path as a clickable breadcrumb. The
 * parent section is null when the focus is an orphan; the
 * orchestrator does NOT mount this component in that case (it
 * collapses the region to zero height).
 */
import type { GraphNode } from "../../api/types";

export interface ParentBreadcrumbProps {
  /** The parent file node from the contextual response. */
  parent: GraphNode;
  /** Called when the user clicks the breadcrumb. */
  onFocus: (id: string) => void;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function ParentBreadcrumb({
  parent,
  onFocus,
  className,
}: ParentBreadcrumbProps) {
  return (
    <div
      data-testid="parent-breadcrumb"
      className={className}
      style={{
        fontSize: 11,
        color: "var(--color-text-secondary)",
        display: "flex",
        gap: 4,
        alignItems: "center",
      }}
    >
      <span aria-hidden="true">in</span>
      <button
        type="button"
        data-testid="parent-breadcrumb-button"
        onClick={() => onFocus(parent.id)}
        style={{
          font: "inherit",
          color: "var(--color-primary)",
          background: "none",
          border: "none",
          padding: 0,
          cursor: "pointer",
          textDecoration: "underline",
        }}
      >
        {parent.label}
      </button>
    </div>
  );
}
