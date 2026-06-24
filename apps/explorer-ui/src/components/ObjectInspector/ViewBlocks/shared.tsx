/**
 * Shared primitives used across multiple ViewBlock renderers.
 */
import type { BlockShellProps } from "./types";

// ============================================================================
// Block shell — wraps every renderer with a header + body
// ============================================================================

export function BlockShell({ id, title, children, testId }: BlockShellProps) {
  return (
    <section
      data-testid={testId ?? `view-block-${id}`}
      data-block-id={id}
      className="flex flex-col gap-1 rounded-md p-3"
      style={{
        backgroundColor: "var(--color-surface-raised)",
        border: "1px solid var(--color-border)",
      }}
    >
      <h3
        className="text-xs font-semibold uppercase tracking-wide"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {title}
      </h3>
      <div className="text-sm" style={{ color: "var(--color-text-primary)" }}>
        {children}
      </div>
    </section>
  );
}

// ============================================================================
// Severity / rating helpers
// ============================================================================

/** Severity color for the per-severity count cells in the summary. */
// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
export function severityTextColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case "blocker":
    case "critical":
      return "var(--color-severity-critical)";
    case "major":
      return "var(--color-severity-high)";
    case "minor":
      return "var(--color-severity-medium)";
    case "info":
    default:
      return "var(--color-severity-low)";
  }
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
export function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case "blocker":
    case "critical":
      return "var(--color-severity-critical)";
    case "high":
    case "warning":
    case "major":
      return "var(--color-severity-high)";
    case "medium":
    case "info":
    case "minor":
      return "var(--color-severity-medium)";
    case "low":
    default:
      return "var(--color-severity-low)";
  }
}

// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
export function ratingColor(rating: string | null): string {
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

/** Trim an ISO timestamp to HH:MM:SS for compact display. */
// eslint-disable-next-line react-refresh/only-export-components -- intentional co-location of helpers; refactor deferred
export function formatLastRun(iso: string): string {
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toISOString().slice(11, 19);
  } catch {
    return iso;
  }
}

// ============================================================================
// Small primitives shared across renderers
// ============================================================================

export function Stat({
  label,
  value,
  small = false,
}: {
  label: string;
  value: number | string;
  small?: boolean;
}) {
  return (
    <div
      className={
        "flex items-center justify-between rounded-sm px-2 py-1" +
        (small ? " text-xs" : "")
      }
      style={{ backgroundColor: "var(--color-surface-overlay)" }}
    >
      <dt style={{ color: "var(--color-text-secondary)" }}>{label}</dt>
      <dd
        className="font-mono"
        style={{ color: "var(--color-text-primary)" }}
      >
        {value}
      </dd>
    </div>
  );
}

export function SeverityChip({ severity }: { severity: string }) {
  const color = severityColor(severity);
  return (
    <span
      aria-label={`Severity ${severity}`}
      className="inline-flex h-5 flex-none items-center rounded-full px-1.5 font-mono text-xs"
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
