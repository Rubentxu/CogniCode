/**
 * `QualityDashboard` — per-scope quality summary panel.
 *
 * Renders a quality summary inside a view (one of the 27 block
 * kinds is `quality_summary`; the `QualityDashboard` exposes a
 * presentational wrapper that buckets issues by severity with
 * click-to-filter chips).
 *
 * This file exports:
 * - `QualityDashboard` — the wrapper component used by the
 *   Object Inspector when the active view contains a
 *   `quality_summary` block.
 * - `SeverityFilterChips` — five colored cards (one per severity)
 *   that the user can click to filter the related `issues_list`
 *   block to that severity.
 *
 * Phase 10 acceptance: A-E rating, debt, per-severity counts.
 */
import { useMemo, useState } from "react";

import type {
  QualityIssueItem,
  QualitySeverity,
  QualitySummaryBlockBody,
  SeverityCounts,
} from "../api/types";

// ============================================================================
// QualityDashboard
// ============================================================================

export interface QualityDashboardProps {
  /**
   * The body of the `quality_summary` block to render. Owns the
   * rating + debt + per-severity counts.
   */
  summary: QualitySummaryBlockBody;
  /**
   * Optional issues list. When provided, the severity chips can
   * filter it. Without it, the chips are visual-only.
   */
  issues?: QualityIssueItem[];
  /**
   * Optional title (e.g. "Scope quality"). Defaults to the body
   * `scope` field.
   */
  title?: string;
}

/**
 * The dashboard. Layout:
 * ┌─────────────────────────────────────┐
 * │ Rating · Debt · Total issues        │
 * ├─────────────────────────────────────┤
 * │ [Blocker] [Critical] [Major] [Min] [Info] │  ← severity chips
 * ├─────────────────────────────────────┤
 * │ Issues list (filtered)              │
 * └─────────────────────────────────────┘
 *
 * Clicking a severity chip toggles a filter on the issues list.
 * Clicking the active chip again clears the filter.
 */
export function QualityDashboard({
  summary,
  issues = [],
  title,
}: QualityDashboardProps) {
  const [activeSeverity, setActiveSeverity] =
    useState<QualitySeverity | null>(null);
  const filteredIssues = useMemo(() => {
    if (!activeSeverity) return issues;
    return issues.filter(
      (it) => it.severity.toLowerCase() === activeSeverity,
    );
  }, [issues, activeSeverity]);

  return (
    <section
      data-testid="quality-dashboard"
      className="flex flex-col gap-3 rounded-md p-3"
      style={{
        backgroundColor: "var(--color-surface-raised)",
        border: "1px solid var(--color-border)",
      }}
    >
      <QualitySummaryHeader summary={summary} title={title} />
      <SeverityFilterChips
        counts={summary.by_severity}
        active={activeSeverity}
        onChange={setActiveSeverity}
      />
      {issues.length > 0 && (
        <FilteredIssuesList
          issues={filteredIssues}
          active={activeSeverity}
          totalCount={issues.length}
        />
      )}
    </section>
  );
}

// ============================================================================
// QualitySummaryHeader — rating + debt + total
// ============================================================================

interface QualitySummaryHeaderProps {
  summary: QualitySummaryBlockBody;
  title?: string;
}

function QualitySummaryHeader({ summary, title }: QualitySummaryHeaderProps) {
  const rating = summary.rating ?? "—";
  return (
    <header className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <h3
          className="truncate text-xs font-semibold uppercase tracking-wide"
          style={{ color: "var(--color-text-secondary)" }}
          title={title ?? summary.scope}
        >
          {title ?? summary.scope}
        </h3>
        <span
          aria-label={`Rating ${rating}`}
          data-testid="quality-dashboard-rating"
          className="inline-flex h-7 w-7 items-center justify-center rounded-md font-mono text-sm"
          style={{
            backgroundColor: ratingColor(summary.rating),
            color: "var(--color-surface)",
          }}
        >
          {rating}
        </span>
      </div>
      <dl className="grid grid-cols-3 gap-2 text-xs">
        <Stat label="Total" value={summary.total_issues} />
        <Stat label="Debt" value={`${summary.debt_minutes} min`} />
        <Stat
          label="Last run"
          value={summary.last_run ? formatTime(summary.last_run) : "—"}
        />
      </dl>
    </header>
  );
}

// ============================================================================
// SeverityFilterChips — five colored cards
// ============================================================================

interface SeverityFilterChipsProps {
  counts: SeverityCounts;
  active: QualitySeverity | null;
  onChange: (next: QualitySeverity | null) => void;
}

const SEVERITIES: QualitySeverity[] = [
  "blocker",
  "critical",
  "major",
  "minor",
  "info",
];

/**
 * Five severity cards. The user can click to filter; clicking the
 * active card again clears the filter. The cards are large and
 * colored (not just chips) because the design calls for them to
 * be the primary "at-a-glance" affordance of the dashboard.
 */
function SeverityFilterChips({
  counts,
  active,
  onChange,
}: SeverityFilterChipsProps) {
  return (
    <div
      role="tablist"
      aria-label="Filter quality issues by severity"
      data-testid="quality-severity-chips"
      className="grid grid-cols-5 gap-1.5"
    >
      {SEVERITIES.map((sev) => {
        const count = counts[sev];
        const isActive = active === sev;
        const color = severityColor(sev);
        return (
          <button
            key={sev}
            type="button"
            role="tab"
            aria-selected={isActive}
            data-testid={`quality-severity-chip-${sev}`}
            onClick={() => onChange(isActive ? null : sev)}
            className="flex flex-col items-center justify-center gap-0.5 rounded-md px-1 py-2 text-xs"
            style={{
              backgroundColor: isActive ? color : "var(--color-surface)",
              color: isActive ? "var(--color-primary-foreground)" : color,
              border: `1px solid ${color}`,
            }}
          >
            <span className="font-mono text-base font-semibold">
              {count}
            </span>
            <span
              className="text-[10px] uppercase tracking-wide"
              style={{
                color: isActive
                  ? "var(--color-primary-foreground)"
                  : "var(--color-text-muted)",
              }}
            >
              {sev}
            </span>
          </button>
        );
      })}
    </div>
  );
}

// ============================================================================
// FilteredIssuesList
// ============================================================================

interface FilteredIssuesListProps {
  issues: QualityIssueItem[];
  active: QualitySeverity | null;
  totalCount: number;
}

function FilteredIssuesList({
  issues,
  active,
  totalCount,
}: FilteredIssuesListProps) {
  return (
    <div className="flex flex-col gap-1">
      <header
        className="flex items-center justify-between text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        <span data-testid="quality-dashboard-issues-count">
          {issues.length}
          {active && ` of ${totalCount}`}{" "}
          {issues.length === 1 ? "issue" : "issues"}
          {active && (
            <>
              {" "}· filtered by <strong>{active}</strong>
            </>
          )}
        </span>
        {active && (
          <button
            type="button"
            onClick={() => {
              /* the parent owns the active state; this is a
                 label only — the parent handles the click. */
            }}
            className="sr-only"
          >
            Clear filter
          </button>
        )}
      </header>
      {issues.length === 0 ? (
        <p
          data-testid="quality-dashboard-empty"
          className="rounded-sm px-2 py-2 text-xs"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-muted)",
          }}
        >
          No {active ?? "issues"} in this scope.
        </p>
      ) : (
        <ul
          data-testid="quality-dashboard-issues"
          className="flex flex-col gap-1"
        >
          {issues.map((it) => (
            <li
              key={it.id}
              data-testid={`quality-dashboard-issue-${it.id}`}
              className="flex flex-col gap-0.5 rounded-sm px-2 py-1 text-xs"
              style={{ backgroundColor: "var(--color-surface-overlay)" }}
            >
              <div className="flex items-center gap-2">
                <SeverityBadge severity={it.severity} />
                <span
                  className="min-w-0 flex-1 truncate font-medium"
                  style={{ color: "var(--color-text-primary)" }}
                  title={it.message}
                >
                  {it.message}
                </span>
              </div>
              <span
                className="font-mono"
                style={{ color: "var(--color-text-muted)" }}
              >
                {it.rule_id} · {it.file}:{it.line}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ============================================================================
// Shared primitives (used by both dashboard and ViewBlock renderers)
// ============================================================================

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <div
      className="flex flex-col rounded-sm px-2 py-1"
      style={{ backgroundColor: "var(--color-surface-overlay)" }}
    >
      <dt
        className="text-[10px] uppercase tracking-wide"
        style={{ color: "var(--color-text-muted)" }}
      >
        {label}
      </dt>
      <dd
        className="font-mono text-sm"
        style={{ color: "var(--color-text-primary)" }}
      >
        {value}
      </dd>
    </div>
  );
}

function SeverityBadge({ severity }: { severity: string }) {
  const color = severityColor(severity);
  return (
    <span
      aria-label={`Severity ${severity}`}
      data-testid={`quality-severity-${severity.toLowerCase()}`}
      className="inline-flex h-5 flex-none items-center rounded-full px-1.5 font-mono text-[10px] uppercase"
      style={{
        backgroundColor: "var(--color-surface)",
        color,
        border: `1px solid ${color}`,
      }}
    >
      {severity}
    </span>
  );
}

// ============================================================================
// Helpers
// ============================================================================

function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case "blocker":
    case "critical":
      return "var(--color-severity-critical)";
    case "high":
    case "major":
      return "var(--color-severity-high)";
    case "medium":
    case "warning":
    case "minor":
      return "var(--color-severity-medium)";
    case "low":
    case "info":
    default:
      return "var(--color-severity-low)";
  }
}

function ratingColor(rating: string | null): string {
  if (!rating) return "var(--color-surface-overlay)";
  switch (rating.toUpperCase()) {
    case "A":
      return "var(--color-success)";
    case "B":
      return "var(--color-primary)";
    case "C":
      return "var(--color-warning)";
    case "D":
    case "E":
    case "F":
      return "var(--color-error)";
    default:
      return "var(--color-surface-overlay)";
  }
}

function formatTime(iso: string): string {
  // Trim to a compact HH:MM:SS for the dashboard header. The full
  // ISO string is still in the body if a user hovers / expands.
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toISOString().slice(11, 19);
  } catch {
    return iso;
  }
}
