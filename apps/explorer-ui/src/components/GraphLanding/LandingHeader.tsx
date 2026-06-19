/**
 * `LandingHeader` — workspace name, symbol count, and scan button.
 *
 * Shown at the top of the GraphLanding view.
 */
import type { WorkspaceSummary } from "../../api/types";

export interface LandingHeaderProps {
  workspace: WorkspaceSummary;
}

export function LandingHeader({ workspace }: LandingHeaderProps) {
  return (
    <div
      data-testid="landing-header"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "12px 16px",
        backgroundColor: "var(--color-surface-raised)",
        borderBottom: "1px solid var(--color-border)",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        <h2
          data-testid="landing-workspace-name"
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--color-text-primary)",
            margin: 0,
          }}
        >
          {workspace.root_path.split("/").pop() ?? workspace.root_path}
        </h2>
        <div
          style={{
            fontSize: 11,
            color: "var(--color-text-muted)",
          }}
        >
          {workspace.symbol_count.toLocaleString()} symbols ·{" "}
          {workspace.relation_count.toLocaleString()} relations
          {workspace.graph_status === "ready" ? (
            <span style={{ color: "var(--color-success)", marginLeft: 8 }}>●</span>
          ) : (
            <span style={{ color: "var(--color-warning)", marginLeft: 8 }}>○</span>
          )}
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span
          data-testid="landing-graph-status"
          style={{
            fontSize: 11,
            padding: "4px 8px",
            borderRadius: 4,
            backgroundColor:
              workspace.graph_status === "ready"
                ? "rgba(16, 185, 129, 0.1)"
                : "rgba(245, 158, 11, 0.1)",
            color:
              workspace.graph_status === "ready"
                ? "var(--color-success)"
                : "var(--color-warning)",
          }}
        >
          {workspace.graph_status}
        </span>
      </div>
    </div>
  );
}
