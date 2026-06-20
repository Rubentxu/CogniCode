/**
 * Quality-related block renderers:
 * SymbolQualityIdentityView, FileQualityIdentityView, ScopeQualityIdentityView,
 * QualityGateView, IssuesListView, IssueIdentityView, IssueLocationView,
 * IssueMessageView, RuleIdentityView, QualitySummaryView, QualityIssueDetailView
 */
import type {
  FileQualityIdentityBlockBody,
  IssuesListBlockBody,
  IssueIdentityBlockBody,
  IssueLocationBlockBody,
  IssueMessageBlockBody,
  QualityGateBlockBody,
  QualityIssueDetailBlockBody,
  QualityIssueItem,
  QualitySummaryBlockBody,
  RuleIdentityBlockBody,
  ScopeQualityIdentityBlockBody,
  SymbolQualityIdentityBlockBody,
  ViewBlock,
} from "../../../api/types";
import { BlockShell, SeverityChip, Stat, formatLastRun, ratingColor, severityTextColor } from "./shared";

// ============================================================================
// Quality identity views
// ============================================================================

export function SymbolQualityIdentityView({
  block,
}: {
  block: ViewBlock & { body: SymbolQualityIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-medium">{b.issue_count}</span>{" "}
        {b.issue_count === 1 ? "issue" : "issues"} at this location
      </p>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {b.file}:{b.line}
      </p>
    </BlockShell>
  );
}

export function FileQualityIdentityView({
  block,
}: {
  block: ViewBlock & { body: FileQualityIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-medium">{b.issue_count}</span>{" "}
        {b.issue_count === 1 ? "issue" : "issues"} in this file
      </p>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {b.path}
      </p>
    </BlockShell>
  );
}

export function ScopeQualityIdentityView({
  block,
}: {
  block: ViewBlock & { body: ScopeQualityIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-medium">{b.issue_count}</span> issues in this
        scope
      </p>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {b.scope}
      </p>
      {Object.keys(b.by_severity).length > 0 && (
        <dl className="mt-1 grid grid-cols-2 gap-1 text-xs">
          {Object.entries(b.by_severity).map(([sev, count]) => (
            <div
              key={sev}
              className="flex items-center justify-between rounded-sm px-2 py-1"
              style={{ backgroundColor: "var(--color-surface-overlay)" }}
            >
              <dt style={{ color: "var(--color-text-secondary)" }}>{sev}</dt>
              <dd className="font-mono">{count}</dd>
            </div>
          ))}
        </dl>
      )}
    </BlockShell>
  );
}

// ============================================================================
// QualityGateView
// ============================================================================

export function QualityGateView({
  block,
}: {
  block: ViewBlock & { body: QualityGateBlockBody };
}) {
  const b = block.body;
  const rating = b.rating ?? "—";
  return (
    <BlockShell id={block.id} title={block.title}>
      <div className="flex items-center gap-3">
        <span
          aria-label={`Rating ${rating}`}
          className="inline-flex h-8 w-8 items-center justify-center rounded-md font-mono text-base"
          style={{
            backgroundColor: ratingColor(b.rating),
            color: "var(--color-surface)",
          }}
        >
          {rating}
        </span>
        <dl className="grid flex-1 grid-cols-2 gap-1 text-xs">
          <Stat label="Total" value={b.total_issues} small />
          <Stat label="Blockers" value={b.blockers} small />
          <Stat label="Criticals" value={b.criticals} small />
          <Stat label="Debt" value={`${b.debt_minutes} min`} small />
        </dl>
      </div>
      {b.last_run && (
        <p
          className="mt-1 font-mono text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Last run: {b.last_run}
        </p>
      )}
    </BlockShell>
  );
}

// ============================================================================
// IssuesListView
// ============================================================================

export function IssuesListView({
  block,
}: {
  block: ViewBlock & { body: IssuesListBlockBody };
}) {
  const items = block.body.items;
  if (items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No issues.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul className="flex flex-col gap-1 text-sm">
        {items.map((it: QualityIssueItem) => (
          <li
            key={it.id}
            data-testid={`view-block-issue-${it.id}`}
            className="flex items-start gap-2 rounded-sm px-2 py-1"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            <SeverityChip severity={it.severity} />
            <div className="min-w-0 flex-1">
              <p
                className="truncate font-medium"
                style={{ color: "var(--color-text-primary)" }}
                title={it.message}
              >
                {it.message}
              </p>
              <p
                className="font-mono text-xs"
                style={{ color: "var(--color-text-muted)" }}
              >
                {it.rule_id} · {it.file}:{it.line}
              </p>
            </div>
          </li>
        ))}
      </ul>
    </BlockShell>
  );
}

// ============================================================================
// Issue identity / location / message
// ============================================================================

export function IssueIdentityView({
  block,
}: {
  block: ViewBlock & { body: IssueIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-mono">#{b.id}</span> ·{" "}
        <span className="font-medium">{b.rule_id}</span>
      </p>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        {b.category} · {b.status}
      </p>
      <SeverityChip severity={b.severity} />
    </BlockShell>
  );
}

export function IssueLocationView({
  block,
}: {
  block: ViewBlock & { body: IssueLocationBlockBody };
}) {
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {block.body.file}:{block.body.line}
      </p>
    </BlockShell>
  );
}

export function IssueMessageView({
  block,
}: {
  block: ViewBlock & { body: IssueMessageBlockBody };
}) {
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>{block.body.message}</p>
    </BlockShell>
  );
}

// ============================================================================
// RuleIdentityView
// ============================================================================

export function RuleIdentityView({
  block,
}: {
  block: ViewBlock & { body: RuleIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-mono">{b.rule_id}</span>
      </p>
      <p style={{ color: "var(--color-text-secondary)" }}>{b.description}</p>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        {b.open_count} open {b.open_count === 1 ? "issue" : "issues"}
      </p>
    </BlockShell>
  );
}

// ============================================================================
// QualitySummaryView
// ============================================================================

/**
 * `quality_summary` — a compact card showing the scope's overall
 * quality rating, debt, total, and per-severity counts.
 */
export function QualitySummaryView({
  block,
}: {
  block: ViewBlock & { body: QualitySummaryBlockBody };
}) {
  const b = block.body;
  const rating = b.rating ?? "—";
  return (
    <BlockShell id={block.id} title={block.title}>
      <div
        data-testid="quality-summary-card"
        className="flex items-center gap-3"
      >
        <span
          aria-label={`Rating ${rating}`}
          data-testid="quality-summary-rating"
          className="inline-flex h-8 w-8 items-center justify-center rounded-md font-mono text-base"
          style={{
            backgroundColor: ratingColor(b.rating),
            color: "var(--color-surface)",
          }}
        >
          {rating}
        </span>
        <dl className="grid flex-1 grid-cols-3 gap-1 text-xs">
          <Stat label="Total" value={b.total_issues} small />
          <Stat label="Debt" value={`${b.debt_minutes} min`} small />
          <Stat
            label="Last run"
            value={b.last_run ? formatLastRun(b.last_run) : "—"}
            small
          />
        </dl>
      </div>
      <dl className="mt-2 grid grid-cols-5 gap-1 text-xs">
        {(["blocker", "critical", "major", "minor", "info"] as const).map(
          (sev) => (
            <div
              key={sev}
              data-testid={`quality-summary-count-${sev}`}
              className="flex flex-col items-center rounded-sm px-1 py-1"
              style={{ backgroundColor: "var(--color-surface-overlay)" }}
            >
              <dt
                className="text-[10px] uppercase tracking-wide"
                style={{ color: "var(--color-text-muted)" }}
              >
                {sev}
              </dt>
              <dd
                className="font-mono"
                style={{ color: severityTextColor(sev) }}
              >
                {b.by_severity[sev]}
              </dd>
            </div>
          ),
        )}
      </dl>
    </BlockShell>
  );
}

// ============================================================================
// QualityIssueDetailView
// ============================================================================

/**
 * `quality_issue_detail` — one issue at full fidelity with rule reference,
 * location, message, and remediation tip.
 */
export function QualityIssueDetailView({
  block,
  onSelectObject,
}: {
  block: ViewBlock & { body: QualityIssueDetailBlockBody };
  onSelectObject?: (objectId: string) => void;
}) {
  const b = block.body;
  const interactive = Boolean(onSelectObject);
  return (
    <BlockShell id={block.id} title={block.title}>
      <div className="flex flex-col gap-2 text-sm">
        <header className="flex items-center gap-2">
          <SeverityChip severity={b.severity} />
          <span
            className="font-mono"
            style={{ color: "var(--color-text-muted)" }}
          >
            #{b.id}
          </span>
          <span
            className="font-mono"
            style={{ color: "var(--color-text-secondary)" }}
          >
            {b.rule_id}
          </span>
        </header>
        <p style={{ color: "var(--color-text-primary)" }}>{b.message}</p>
        <button
          type="button"
          onClick={
            interactive ? () => onSelectObject?.(b.object_id) : undefined
          }
          onKeyDown={
            interactive
              ? (e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelectObject?.(b.object_id);
                  }
                }
              : undefined
          }
          disabled={!interactive}
          className={
            "self-start rounded-sm px-2 py-1 font-mono text-xs" +
            (interactive ? " cursor-pointer" : "")
          }
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
          data-testid="quality-issue-detail-location"
        >
          {b.file}:{b.line}
        </button>
        {b.rule_description && (
          <div
            data-testid="quality-issue-detail-rule"
            className="rounded-sm p-2 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-secondary)",
            }}
          >
            <p
              className="text-[10px] font-semibold uppercase tracking-wide"
              style={{ color: "var(--color-text-muted)" }}
            >
              Rule
            </p>
            <p>{b.rule_description}</p>
          </div>
        )}
        {b.remediation && (
          <div
            data-testid="quality-issue-detail-remediation"
            className="rounded-sm p-2 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
            }}
          >
            <p
              className="text-[10px] font-semibold uppercase tracking-wide"
              style={{ color: "var(--color-text-muted)" }}
            >
              Remediation
            </p>
            <p>{b.remediation}</p>
          </div>
        )}
        {b.rule_url && (
          <a
            data-testid="quality-issue-detail-rule-url"
            href={b.rule_url}
            target="_blank"
            rel="noreferrer"
            className="text-xs underline"
            style={{ color: "var(--color-primary)" }}
          >
            Rule reference
          </a>
        )}
      </div>
    </BlockShell>
  );
}
