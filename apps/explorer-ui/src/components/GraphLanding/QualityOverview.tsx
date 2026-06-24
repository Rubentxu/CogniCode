/**
 * `QualityOverview` — workspace-wide quality summary section for the
 * GraphLanding page. Wraps the existing QualityDashboard component
 * with data fetching + collapsible UI.
 *
 * PR #35 — moldable wiring phase1.
 */
import { useState, type JSX } from "react";
import useSWR from "swr";

import { QualityDashboard } from "../QualityDashboard";
import type { QualitySummaryBlockBody } from "../../api/types";
import {
  fetchQualitySummary,
  type WorkspaceQualityPayload,
} from "../../api/client";

export function QualityOverview({
  workspaceId,
}: {
  workspaceId: string;
}): JSX.Element | null {
  const [collapsed, setCollapsed] = useState(false);

  const { data, isLoading, error } = useSWR<WorkspaceQualityPayload>(
    workspaceId ? `workspace-quality:${workspaceId}` : null,
    () => fetchQualitySummary(workspaceId),
    { revalidateOnFocus: false },
  );

  return (
    <section
      data-testid="quality-overview"
      className="flex flex-col gap-2 border-t pt-4"
      style={{ borderColor: "var(--color-border)" }}
    >
      <header className="flex items-center justify-between gap-2 px-4">
        <h2
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Workspace Quality
        </h2>
        <button
          type="button"
          data-testid="quality-overview-toggle"
          aria-label={collapsed ? "Expand quality overview" : "Collapse quality overview"}
          aria-expanded={!collapsed}
          onClick={() => setCollapsed((c) => !c)}
          className="rounded-md px-2 py-0.5 text-xs"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-secondary)",
          }}
        >
          {collapsed ? "▸" : "▾"}
        </button>
      </header>
      {!collapsed && (
        <div className="px-4">
          {isLoading && (
            <p
              data-testid="quality-overview-loading"
              className="text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              Loading workspace quality…
            </p>
          )}
          {error && !isLoading && (
            <p
              data-testid="quality-overview-error"
              className="text-xs"
              style={{ color: "var(--color-error)" }}
            >
              Failed to load quality data.
            </p>
          )}
          {!isLoading && !error && (
            <QualityDashboard
              summary={data?.summary ?? EMPTY_SUMMARY}
              issues={data?.issues ?? []}
            />
          )}
        </div>
      )}
    </section>
  );
}

const EMPTY_SUMMARY: QualitySummaryBlockBody = {
  scope: "workspace",
  rating: null,
  total_issues: 0,
  debt_minutes: 0,
  by_severity: {
    blocker: 0,
    critical: 0,
    major: 0,
    minor: 0,
    info: 0,
  },
  last_run: null,
};
