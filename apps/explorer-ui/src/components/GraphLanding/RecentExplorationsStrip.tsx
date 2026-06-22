/**
 * `RecentExplorationsStrip` — shows recent saved explorations below the graph.
 *
 * Displays the user's saved explorations from `useExplorations` and allows
 * clicking to navigate to the first object in that exploration.
 */
import { useExplorations } from "../../hooks/useExplorations";
import type { ExplorationPath } from "../../api/types";

export interface RecentExplorationsStripProps {
  workspaceId: string;
  onExplorationClick: (exploration: ExplorationPath) => void;
}

function ExplorationCard({
  exploration,
  onClick,
}: {
  exploration: ExplorationPath;
  onClick: () => void;
}) {
  // Use the first column's object as the title, or fall back to the ID
  const firstColumn = exploration.columns[0];
  const title = firstColumn?.object_id ?? exploration.id;
  const timestamp = new Date(exploration.created_at).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={`recent-exploration-${exploration.id}`}
      style={{
        padding: "8px 12px",
        borderRadius: 8,
        border: "1px solid var(--color-border)",
        backgroundColor: "var(--color-surface-overlay)",
        color: "var(--color-text-primary)",
        fontSize: 12,
        cursor: "pointer",
        transition: "background-color 0.15s, border-color 0.15s",
        textAlign: "left",
        minWidth: 120,
        maxWidth: 200,
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.backgroundColor = "var(--color-surface)";
        e.currentTarget.style.borderColor = "var(--color-text-muted)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.backgroundColor = "var(--color-surface-overlay)";
        e.currentTarget.style.borderColor = "var(--color-border)";
      }}
    >
      <div
        style={{
          fontWeight: 600,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          marginBottom: 4,
        }}
      >
        {title}
      </div>
      <div
        style={{
          fontSize: 10,
          color: "var(--color-text-muted)",
        }}
      >
        {timestamp} · {exploration.columns.length} pane{exploration.columns.length !== 1 ? "s" : ""}
      </div>
    </button>
  );
}

export function RecentExplorationsStrip({
  workspaceId,
  onExplorationClick,
}: RecentExplorationsStripProps) {
  const { data: explorations, isLoading } = useExplorations(workspaceId);

  if (isLoading) {
    return null;
  }

  if (!explorations || explorations.length === 0) {
    return null;
  }

  // Show at most 5 most recent explorations
  const recentExplorations = [...explorations]
    .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
    .slice(0, 5);

  return (
    <div
      data-testid="recent-explorations-strip"
      style={{
        padding: "12px 16px",
        backgroundColor: "var(--color-surface-raised)",
        borderTop: "1px solid var(--color-border)",
      }}
    >
      <div
        style={{
          fontSize: 11,
          color: "var(--color-text-muted)",
          marginBottom: 8,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
        }}
      >
        Recent Explorations
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {recentExplorations.map((exploration) => (
          <ExplorationCard
            key={exploration.id}
            exploration={exploration}
            onClick={() => onExplorationClick(exploration)}
          />
        ))}
      </div>
    </div>
  );
}