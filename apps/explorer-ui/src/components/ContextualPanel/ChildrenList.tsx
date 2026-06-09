/**
 * `ChildrenList` — third section of the `ContextualPanel`. Renders
 * the sibling symbols of the focus (the children section of the
 * contextual response) as a scrollable, clickable list. Empty when
 * the file has no other symbols.
 */
import type { GraphNode } from "../../api/types";

export interface ChildrenListProps {
  /** Sibling nodes (siblings of the focus in its file). */
  children: GraphNode[];
  /** Called when the user clicks a row. */
  onFocus: (id: string) => void;
  /** Optional CSS class passthrough. */
  className?: string;
  /** Max height for the scrollable region. Defaults to 240px. */
  maxHeight?: number;
}

export function ChildrenList({
  children,
  onFocus,
  className,
  maxHeight = 240,
}: ChildrenListProps) {
  if (children.length === 0) {
    return (
      <div
        data-testid="children-list-empty"
        className={className}
        style={{
          fontSize: 11,
          color: "var(--color-text-muted)",
          padding: "4px 0",
        }}
      >
        No sibling symbols in this file
      </div>
    );
  }
  return (
    <ul
      data-testid="children-list"
      className={className}
      role="list"
      style={{
        listStyle: "none",
        margin: 0,
        padding: 0,
        maxHeight,
        overflowY: "auto",
        border: "1px solid var(--color-border)",
        borderRadius: 4,
        backgroundColor: "var(--color-surface-raised)",
      }}
    >
      {children.map((c) => (
        <li
          key={c.id}
          role="listitem"
          data-testid="children-list-row"
          tabIndex={0}
          onClick={() => onFocus(c.id)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onFocus(c.id);
            }
          }}
          style={{
            padding: "4px 8px",
            fontSize: 11,
            fontFamily: "ui-monospace, monospace",
            cursor: "pointer",
            borderBottom: "1px solid var(--color-border)",
            color: "var(--color-text-primary)",
          }}
        >
          {c.label}
        </li>
      ))}
    </ul>
  );
}
