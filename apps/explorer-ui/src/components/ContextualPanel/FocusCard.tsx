/**
 * `FocusCard` — top section of the `ContextualPanel`. Displays the
 * currently-focused symbol: id, kind, file path.
 *
 * Always visible when the panel is mounted; the panel orchestrator
 * (ContextualPanel) controls the surrounding loading / error
 * states.
 */
import type { GraphNode } from "../../api/types";

export interface FocusCardProps {
  /** The focus node from the contextual response. */
  focus: GraphNode;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function FocusCard({ focus, className }: FocusCardProps) {
  const fileLine =
    focus.file && focus.line !== undefined ? `${focus.file}:${focus.line}` : focus.file ?? "—";
  return (
    <div
      data-testid="focus-card"
      className={className}
      style={{
        padding: "8px 10px",
        borderRadius: 6,
        backgroundColor: "var(--color-surface-raised)",
        border: "1px solid var(--color-border)",
        display: "flex",
        flexDirection: "column",
        gap: 2,
      }}
    >
      <div
        data-testid="focus-card-id"
        style={{
          fontSize: 12,
          fontFamily: "ui-monospace, monospace",
          color: "var(--color-text-primary)",
          wordBreak: "break-all",
        }}
      >
        {focus.id}
      </div>
      <div
        style={{
          display: "flex",
          gap: 8,
          fontSize: 11,
          color: "var(--color-text-secondary)",
        }}
      >
        <span data-testid="focus-card-kind">{focus.kind}</span>
        <span aria-hidden="true">·</span>
        <span data-testid="focus-card-file">{fileLine}</span>
      </div>
    </div>
  );
}
