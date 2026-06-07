/**
 * `LensPanel` — the rightmost panel of the Explorer.
 *
 * The panel:
 * 1. Lists the lenses applicable to the currently active object.
 * 2. Lets the user pick a lens, then triggers `useLensResult` to
 *    fetch the `DesignFinding[]` for it.
 * 3. Groups findings by severity, color-codes the chips, and shows
 *    the confidence as a percentage + bar.
 * 4. Has a "blockers only" filter toggle.
 * 5. Clicking a finding navigates to the first affected object
 *    (dispatches `SELECT_OBJECT` with that id).
 *
 * The panel is layout-only — the only stateful bits are the
 * `selectedLensId` (per-panel local) and `blockerOnly` filter
 * (also local). The active lens id is *not* lifted to the global
 * AppContext because the UI may show one lens per object across
 * navigations; keeping the selection local keeps the back-button
 * experience predictable without giving up the per-object memory.
 */
import { useEffect, useMemo, useRef, useState } from "react";

import { useApp, useAppDispatch } from "../state/context";
import { useLenses } from "../hooks/useLenses";
import { useLensResult } from "../hooks/useLensResult";
import { LoadingTier } from "./LoadingTier";
import { ErrorBoundary } from "./ErrorBoundary";
import type { DesignFinding, FindingSeverity } from "../api/types";

// ============================================================================
// Severity ordering — blockers first, info last
// ============================================================================

/**
 * Order findings from highest to lowest impact. `FindingSeverity`
 * is the broader QualitySeverity ("blocker"/"critical"/"major"/
 * "minor"/"info") when emitted by the dashboard, but the design
 * lens itself returns "info" | "warning" | "critical". We normalise
 * both into a single rank so the ordering is consistent.
 */
const SEVERITY_RANK: Record<string, number> = {
  blocker: 0,
  critical: 1,
  major: 2,
  warning: 3,
  minor: 4,
  info: 5,
};

function severityRank(severity: string): number {
  return SEVERITY_RANK[severity.toLowerCase()] ?? 99;
}

/**
 * A finding's "object_ids" carry one or more ids it relates to.
 * We navigate to the first one — keeps the click target unambiguous
 * and matches the design's "click finding to inspect the location"
 * intent.
 */
function firstObjectId(finding: DesignFinding): string | null {
  return finding.object_ids[0] ?? null;
}

// ============================================================================
// Public component
// ============================================================================

export function LensPanel() {
  const { state } = useApp();
  const dispatch = useAppDispatch();
  const { activeObjectId, activeLensId } = state;

  // The local selection lives here, but we honour the AppContext's
  // `activeLensId` on first mount so a saved exploration or a deep
  // link can re-open the same lens.
  const [selectedLensId, setSelectedLensId] = useState<string | null>(
    activeLensId,
  );
  const [blockerOnly, setBlockerOnly] = useState(false);

  // Mirror the local selection back to the reducer so the Spotter /
  // other panels can read the active lens.
  useEffect(() => {
    dispatch({ type: "SET_ACTIVE_LENS", payload: { lensId: selectedLensId } });
  }, [selectedLensId, dispatch]);

  // Reset the local selection when the user navigates to a new
  // object. A new object means the available lenses are different
  // and the previously selected lens is very likely no longer
  // applicable.
  //
  // We only clear the selection on a *transition* (previous
  // objectId !== current objectId), NOT on the first mount. That
  // way a saved exploration that sets `activeLensId` keeps its
  // selection when the panel first appears.
  const previousObjectIdRef = useRef<string | null>(activeObjectId);
  useEffect(() => {
    if (
      previousObjectIdRef.current !== null &&
      previousObjectIdRef.current !== activeObjectId
    ) {
      setSelectedLensId(null);
    }
    previousObjectIdRef.current = activeObjectId;
  }, [activeObjectId]);

  // Track the most recent finding count so the header badge can
  // show "3" instead of hiding. We surface 0 while loading so the
  // user gets visual feedback that the lens is in flight.
  const [findingCount, setFindingCount] = useState<number | null>(null);

  if (!activeObjectId) {
    return <EmptyState>Select an object to see its lenses.</EmptyState>;
  }

  return (
    <ErrorBoundary label="LensPanel">
      <div
        data-testid="lens-panel"
        className="flex h-full flex-col"
        style={{ backgroundColor: "var(--color-surface)" }}
      >
        <Header
          findingCount={findingCount}
          onToggleBlockers={() => setBlockerOnly((v) => !v)}
          blockerOnly={blockerOnly}
        />
        <LensesList
          activeObjectId={activeObjectId}
          selectedLensId={selectedLensId}
          onSelectLens={setSelectedLensId}
        />
        {selectedLensId && (
          <LensResult
            activeObjectId={activeObjectId}
            lensId={selectedLensId}
            blockerOnly={blockerOnly}
            onCount={setFindingCount}
            onPickFinding={(finding) => {
              const target = firstObjectId(finding);
              if (!target) return;
              dispatch({
                type: "SELECT_OBJECT",
                payload: { objectId: target, viewId: "overview" },
              });
            }}
          />
        )}
      </div>
    </ErrorBoundary>
  );
}

// ============================================================================
// Header
// ============================================================================

interface HeaderProps {
  findingCount: number | null;
  onToggleBlockers: () => void;
  blockerOnly: boolean;
}

function Header({ findingCount, onToggleBlockers, blockerOnly }: HeaderProps) {
  return (
    <header
      className="flex items-center justify-between gap-2 px-4 py-2"
      style={{ borderBottom: "1px solid var(--color-border)" }}
    >
      <h2
        className="truncate text-sm font-semibold"
        style={{ color: "var(--color-text-primary)" }}
      >
        Lenses
      </h2>
      <div className="flex items-center gap-2">
        {findingCount !== null && (
          <span
            data-testid="lens-finding-count"
            className="rounded-full px-2 py-0.5 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-muted)",
            }}
          >
            {findingCount}
          </span>
        )}
        <button
          type="button"
          role="switch"
          aria-checked={blockerOnly}
          aria-label="Show blockers only"
          data-testid="lens-blocker-toggle"
          onClick={onToggleBlockers}
          className="rounded-full px-2 py-0.5 text-xs font-medium"
          style={{
            backgroundColor: blockerOnly
              ? "var(--color-error)"
              : "var(--color-surface-overlay)",
            color: blockerOnly
              ? "var(--color-primary-foreground)"
              : "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          Blockers only
        </button>
      </div>
    </header>
  );
}

// ============================================================================
// LensesList — picks from the available lenses
// ============================================================================

interface LensesListProps {
  activeObjectId: string;
  selectedLensId: string | null;
  onSelectLens: (id: string) => void;
}

function LensesList({
  activeObjectId,
  selectedLensId,
  onSelectLens,
}: LensesListProps) {
  const { data, isLoading, isValidating, error } = useLenses(activeObjectId);

  if (error) {
    return (
      <LoadingTier
        data={null}
        isLoading={false}
        error={error}
        label="Available lenses"
      >
        <div />
      </LoadingTier>
    );
  }

  return (
    <LoadingTier
      data={data}
      isLoading={isLoading && !data}
      isValidating={isValidating}
      label="Available lenses"
    >
      <ul
        data-testid="lens-list"
        tabIndex={0}
        className="overflow-y-auto border-b p-2 text-sm"
        style={{
          borderColor: "var(--color-border)",
          color: "var(--color-text-secondary)",
        }}
      >
        {data?.map((lens) => {
          const active = lens.id === selectedLensId;
          return (
            <li key={lens.id}>
              <button
                type="button"
                onClick={() => onSelectLens(lens.id)}
                data-testid={`lens-item-${lens.id}`}
                aria-pressed={active}
                className="flex w-full flex-col items-start gap-0.5 rounded-sm px-2 py-1.5 text-left"
                style={{
                  backgroundColor: active
                    ? "var(--color-primary)"
                    : "transparent",
                  color: active
                    ? "var(--color-primary-foreground)"
                    : "var(--color-text-primary)",
                }}
              >
                <span className="text-sm font-medium">{lens.name}</span>
                <span
                  className="text-xs"
                  style={{
                    color: active
                      ? "var(--color-primary-foreground)"
                      : "var(--color-text-muted)",
                  }}
                >
                  {lens.description}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </LoadingTier>
  );
}

// ============================================================================
// LensResult — fetches + renders the findings
// ============================================================================

interface LensResultProps {
  activeObjectId: string;
  lensId: string;
  blockerOnly: boolean;
  onCount: (count: number | null) => void;
  onPickFinding: (finding: DesignFinding) => void;
}

function LensResult({
  activeObjectId,
  lensId,
  blockerOnly,
  onCount,
  onPickFinding,
}: LensResultProps) {
  const { data, isLoading, isValidating, error, mutate } = useLensResult({
    objectId: activeObjectId,
    lensId,
  });

  // Surface the count up to the parent so the header badge stays
  // in sync. We report 0 while the call is in flight so the user
  // can see something happening, and null on error so the badge
  // hides.
  useEffect(() => {
    if (error) {
      onCount(null);
    } else if (data) {
      onCount(data.findings.length);
    } else if (isLoading) {
      onCount(0);
    }
  }, [data, isLoading, error, onCount]);

  if (error) {
    return (
      <div
        role="alert"
        data-testid="lens-result-error"
        className="flex flex-1 flex-col items-center justify-center gap-2 p-4 text-center text-sm"
        style={{ color: "var(--color-error)" }}
      >
        <span className="font-semibold">Failed to apply lens</span>
        <span
          className="text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          {error.message}
        </span>
        <button
          type="button"
          onClick={() => void mutate()}
          data-testid="lens-result-retry"
          className="mt-1 rounded-md px-2 py-1 text-xs"
          style={{
            backgroundColor: "var(--color-primary)",
            color: "var(--color-primary-foreground)",
          }}
        >
          Retry
        </button>
      </div>
    );
  }

  if (isLoading && !data) {
    return (
      <div
        role="status"
        aria-busy="true"
        aria-label="Applying lens"
        data-testid="lens-result-loading"
        className="flex flex-1 items-center justify-center p-4 text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        Applying lens…
      </div>
    );
  }

  if (!data) {
    return null;
  }

  if (data.findings.length === 0) {
    return (
      <div
        data-testid="lens-result-empty"
        className="flex flex-1 flex-col items-start gap-1 p-4 text-sm"
        style={{ color: "var(--color-text-muted)" }}
      >
        <p className="font-medium">No findings</p>
        <p className="text-xs">{data.summary || "This lens found nothing."}</p>
      </div>
    );
  }

  return (
    <LensFindingsView
      findings={data.findings}
      summary={data.summary}
      blockerOnly={blockerOnly}
      isValidating={isValidating}
      onPickFinding={onPickFinding}
    />
  );
}

// ============================================================================
// LensFindingsView — groups, filters, renders the DesignFinding list
// ============================================================================

interface LensFindingsViewProps {
  findings: DesignFinding[];
  summary: string;
  blockerOnly: boolean;
  isValidating: boolean;
  onPickFinding: (finding: DesignFinding) => void;
}

function LensFindingsView({
  findings,
  summary,
  blockerOnly,
  isValidating,
  onPickFinding,
}: LensFindingsViewProps) {
  // Filter for the "blockers only" toggle. We treat both `blocker`
  // and `critical` severities as blockers — they are the ones the
  // user is most likely to want to triage first.
  const filtered = useMemo(() => {
    if (!blockerOnly) return findings;
    return findings.filter(
      (f) =>
        f.severity.toLowerCase() === "blocker" ||
        f.severity.toLowerCase() === "critical",
    );
  }, [findings, blockerOnly]);

  const grouped = useMemo(() => groupBySeverity(filtered), [filtered]);

  return (
    <div
      data-testid="lens-result"
      aria-busy={isValidating}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {summary && (
        <p
          data-testid="lens-result-summary"
          className="border-b px-3 py-2 text-xs"
          style={{
            color: "var(--color-text-muted)",
            borderColor: "var(--color-border)",
          }}
        >
          {summary}
        </p>
      )}
      <div
        data-testid="lens-result-groups"
        tabIndex={0}
        className="flex-1 overflow-y-auto p-2"
      >
        {grouped.map((group) => (
          <FindingsGroup
            key={group.severity}
            severity={group.severity}
            findings={group.findings}
            onPickFinding={onPickFinding}
          />
        ))}
        {filtered.length === 0 && (
          <p
            data-testid="lens-result-blocker-empty"
            className="p-2 text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            No blockers — toggle off to see all findings.
          </p>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// FindingsGroup — one severity bucket
// ============================================================================

interface FindingsGroupProps {
  severity: FindingSeverity;
  findings: DesignFinding[];
  onPickFinding: (finding: DesignFinding) => void;
}

function FindingsGroup({
  severity,
  findings,
  onPickFinding,
}: FindingsGroupProps) {
  return (
    <section
      data-testid={`lens-group-${severity.toLowerCase()}`}
      className="mb-2"
    >
      <header
        className="mb-1 flex items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wide"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <SeverityBadge severity={severity} />
        <span aria-hidden="true">·</span>
        <span>{findings.length}</span>
      </header>
      <ul className="flex flex-col gap-1">
        {findings.map((finding) => (
          <FindingRow
            key={finding.id}
            finding={finding}
            onClick={() => onPickFinding(finding)}
          />
        ))}
      </ul>
    </section>
  );
}

// ============================================================================
// FindingRow — one design finding, clickable
// ============================================================================

interface FindingRowProps {
  finding: DesignFinding;
  onClick: () => void;
}

function FindingRow({ finding, onClick }: FindingRowProps) {
  const target = firstObjectId(finding);
  return (
    <li>
      <button
        type="button"
        onClick={onClick}
        disabled={!target}
        data-testid={`lens-finding-${finding.id}`}
        className="flex w-full flex-col gap-1 rounded-sm px-2 py-1.5 text-left text-sm"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          border: "1px solid var(--color-border)",
          color: "var(--color-text-primary)",
        }}
      >
        <span className="font-medium">{finding.title}</span>
        {finding.hypothesis && (
          <span
            className="text-xs"
            style={{ color: "var(--color-text-secondary)" }}
          >
            {finding.hypothesis}
          </span>
        )}
        <ConfidenceBar confidence={finding.confidence} />
      </button>
    </li>
  );
}

// ============================================================================
// Small primitives
// ============================================================================

function SeverityBadge({ severity }: { severity: string }) {
  const color = severityColor(severity);
  return (
    <span
      aria-label={`Severity ${severity}`}
      data-testid={`lens-severity-${severity.toLowerCase()}`}
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

function ConfidenceBar({ confidence }: { confidence: number }) {
  const pct = Math.round(confidence * 100);
  const colour = confidenceColor(confidence);
  return (
    <div
      data-testid="lens-confidence"
      className="flex items-center gap-2"
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
      aria-label={`Confidence ${pct}%`}
    >
      <div
        className="relative h-1.5 flex-1 overflow-hidden rounded-full"
        style={{ backgroundColor: "var(--color-surface-overlay)" }}
      >
        <div
          className="absolute left-0 top-0 h-full rounded-full"
          style={{
            width: `${pct}%`,
            backgroundColor: colour,
          }}
        />
      </div>
      <span
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {pct}%
      </span>
    </div>
  );
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div
      data-testid="lens-panel-empty"
      className="flex h-full items-center justify-center p-6 text-center text-sm"
      style={{ color: "var(--color-text-secondary)" }}
    >
      <p>{children}</p>
    </div>
  );
}

// ============================================================================
// Helpers
// ============================================================================

interface GroupBucket {
  severity: FindingSeverity;
  findings: DesignFinding[];
}

/**
 * Group findings by severity. Output is a stable order:
 * blockers → criticals → majors → warnings → minors → info.
 * Severities that don't appear in the result set are skipped
 * (no empty headers).
 */
function groupBySeverity(findings: DesignFinding[]): GroupBucket[] {
  const buckets = new Map<string, DesignFinding[]>();
  for (const f of findings) {
    const key = f.severity;
    const list = buckets.get(key) ?? [];
    list.push(f);
    buckets.set(key, list);
  }
  const keys = Array.from(buckets.keys()).sort(
    (a, b) => severityRank(a) - severityRank(b),
  );
  return keys.map((k) => ({
    severity: k as FindingSeverity,
    findings: buckets.get(k) ?? [],
  }));
}

function severityColor(severity: string): string {
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

function confidenceColor(confidence: number): string {
  if (confidence >= 0.7) return "var(--color-success)";
  if (confidence >= 0.4) return "var(--color-warning)";
  return "var(--color-error)";
}
