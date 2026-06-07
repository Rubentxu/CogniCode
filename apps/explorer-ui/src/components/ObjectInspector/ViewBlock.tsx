/**
 * `ViewBlock` — render a single block from a `ContextualView`.
 *
 * The `ViewBlock` discriminated union covers 27 known block ids
 * plus a permissive fallback. We switch on `block.id` and pick
 * the right renderer. The fallback renders raw JSON so the UI
 * keeps working when a new block kind ships before its renderer.
 *
 * Why hand-rolled, not a single `lookup` map?
 * - Each block has its own layout. A switch keeps the visual
 *   co-located with the type, which is the right shape for
 *   reviewability.
 * - TypeScript's `never` exhaustiveness check on the default
 *   branch gives us a compile-time guard: the moment a new block
 *   id is added to `viewBlockSchema`, the type narrows in this
 *   switch and the fallback branch breaks. That's a feature.
 */
import { useMemo } from "react";

import type {
  CallListBlockBody,
  ContextualView,
  CrossScopeBlockBody,
  FileIdentityBlockBody,
  FileSymbolsBlockBody,
  HotspotsBlockBody,
  IdentityBlockBody,
  IssuesListBlockBody,
  IssueIdentityBlockBody,
  IssueLocationBlockBody,
  IssueMessageBlockBody,
  KindsBreakdownBlockBody,
  QualityGateBlockBody,
  QualityIssueItem,
  RelationItem,
  RuleIdentityBlockBody,
  ScopeFilesBlockBody,
  ScopeIdentityBlockBody,
  SourceLine,
  SourceSliceBlockBody,
  SymbolQualityIdentityBlockBody,
  FileQualityIdentityBlockBody,
  ScopeQualityIdentityBlockBody,
  QualitySummaryBlockBody,
  QualityIssueDetailBlockBody,
  UnknownViewBlock,
  ViewBlock,
} from "../../api/types";

// ============================================================================
// Public component
// ============================================================================

export interface ViewBlockProps {
  /** The block to render. We accept `ViewBlockAny` so unknown blocks fall through. */
  block: ViewBlock | UnknownViewBlock;
  /**
   * Optional callback when the user picks a related object (a
   * caller / callee / hotspot etc). When present, those items
   * become interactive.
   */
  onSelectObject?: (objectId: string) => void;
}

/**
 * Route a block to its renderer. The exhaustive `never` check on
 * the default branch is intentional — it is the "you forgot a
 * renderer" compile-time guard described in the file header.
 */
export function ViewBlock({ block, onSelectObject }: ViewBlockProps) {
  // Unknown blocks (id is not in the typed union) land in the
  // fallback. The schema for `ViewBlockAny` collapses to the
  // typed union OR the permissive shape; we narrow here.
  const id = (block as { id: string }).id;
  const isKnown = isKnownBlockId(id);
  if (!isKnown) {
    return <UnknownBlockView block={block as UnknownViewBlock} />;
  }

  // The narrowing happens via `id`. Inside the switch, we cast
  // `block` to the precise body shape for the matched id.
  switch (id) {
    case "identity":
      return <IdentityView block={typed<typeof block, "identity">(block)} />;
    case "call_metrics":
      return <CallMetricsView block={typed<typeof block, "call_metrics">(block)} />;
    case "signature":
      return <SignatureView block={typed<typeof block, "signature">(block)} />;
    case "callers":
      return (
        <CallListView
          block={typed<typeof block, "callers">(block)}
          onSelectObject={onSelectObject}
        />
      );
    case "callees":
      return (
        <CallListView
          block={typed<typeof block, "callees">(block)}
          onSelectObject={onSelectObject}
        />
      );
    case "source_slice":
      return <SourceView block={typed<typeof block, "source_slice">(block)} />;
    case "symbol_quality_identity":
      return (
        <SymbolQualityIdentityView
          block={typed<typeof block, "symbol_quality_identity">(block)}
        />
      );
    case "symbol_quality_issues":
      return <IssuesListView block={typed<typeof block, "symbol_quality_issues">(block)} />;
    case "file_quality_identity":
      return (
        <FileQualityIdentityView
          block={typed<typeof block, "file_quality_identity">(block)}
        />
      );
    case "file_quality_issues":
      return <IssuesListView block={typed<typeof block, "file_quality_issues">(block)} />;
    case "file_quality_gate":
      return <QualityGateView block={typed<typeof block, "file_quality_gate">(block)} />;
    case "scope_quality_identity":
      return (
        <ScopeQualityIdentityView
          block={typed<typeof block, "scope_quality_identity">(block)}
        />
      );
    case "scope_quality_gate":
      return <QualityGateView block={typed<typeof block, "scope_quality_gate">(block)} />;
    case "scope_quality_issues":
      return <IssuesListView block={typed<typeof block, "scope_quality_issues">(block)} />;
    case "issue_identity":
      return <IssueIdentityView block={typed<typeof block, "issue_identity">(block)} />;
    case "issue_location":
      return <IssueLocationView block={typed<typeof block, "issue_location">(block)} />;
    case "issue_message":
      return <IssueMessageView block={typed<typeof block, "issue_message">(block)} />;
    case "rule_identity":
      return <RuleIdentityView block={typed<typeof block, "rule_identity">(block)} />;
    case "rule_related":
      return <IssuesListView block={typed<typeof block, "rule_related">(block)} />;
    case "file_identity":
      return <FileIdentityView block={typed<typeof block, "file_identity">(block)} />;
    case "kinds":
      return <KindsView block={typed<typeof block, "kinds">(block)} />;
    case "symbols":
      return <FileSymbolsView block={typed<typeof block, "symbols">(block)} />;
    case "scope_identity":
      return <ScopeIdentityView block={typed<typeof block, "scope_identity">(block)} />;
    case "scope_kinds":
      return <KindsView block={typed<typeof block, "scope_kinds">(block)} />;
    case "scope_files":
      return <ScopeFilesView block={typed<typeof block, "scope_files">(block)} />;
    case "cross_scope":
      return <CrossScopeView block={typed<typeof block, "cross_scope">(block)} />;
    case "hotspots":
      return (
        <HotspotsView
          block={typed<typeof block, "hotspots">(block)}
          onSelectObject={onSelectObject}
        />
      );
    case "quality_summary":
      return (
        <QualitySummaryView
          block={typed<typeof block, "quality_summary">(block)}
        />
      );
    case "quality_issue_detail":
      return (
        <QualityIssueDetailView
          block={typed<typeof block, "quality_issue_detail">(block)}
          onSelectObject={onSelectObject}
        />
      );
    default: {
      // Exhaustiveness check. If a new id is added to the union,
      // TypeScript narrows `id` to the new string and the cast
      // below is impossible — the compiler complains. The fix is
      // to add a new case above.
      const _exhaustive: never = id;
      void _exhaustive;
      return <UnknownBlockView block={block as UnknownViewBlock} />;
    }
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Cast a `ViewBlock | UnknownViewBlock` to one of the typed shapes
 * keyed by `id`. We re-narrow with a discriminant check first to
 * keep the rest of the file sound. The `unknown` intermediate is
 * necessary because TS cannot correlate `id` to the body shape
 * automatically when the block itself is widened.
 */
function typed<B extends { id: string }, K extends ViewBlock["id"]>(
  block: B,
): Extract<ViewBlock, { id: K }> {
  return block as unknown as Extract<ViewBlock, { id: K }>;
}

const KNOWN_IDS = new Set<string>([
  "identity",
  "call_metrics",
  "signature",
  "callers",
  "callees",
  "source_slice",
  "symbol_quality_identity",
  "symbol_quality_issues",
  "file_quality_identity",
  "file_quality_issues",
  "file_quality_gate",
  "scope_quality_identity",
  "scope_quality_gate",
  "scope_quality_issues",
  "issue_identity",
  "issue_location",
  "issue_message",
  "rule_identity",
  "rule_related",
  "file_identity",
  "kinds",
  "symbols",
  "scope_identity",
  "scope_kinds",
  "scope_files",
  "cross_scope",
  "hotspots",
  "quality_summary",
  "quality_issue_detail",
]);

function isKnownBlockId(id: string): id is ViewBlock["id"] {
  return KNOWN_IDS.has(id);
}

// ============================================================================
// Block shell — wraps every renderer with a header + body
// ============================================================================

interface BlockShellProps {
  id: string;
  title: string;
  children: React.ReactNode;
  testId?: string;
}

function BlockShell({ id, title, children, testId }: BlockShellProps) {
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
// Renderers — one per known block id
// ============================================================================

function IdentityView({ block }: { block: ViewBlock & { body: IdentityBlockBody } }) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-semibold">{b.name}</span>{" "}
        <span style={{ color: "var(--color-text-muted)" }}>· {b.kind}</span>
      </p>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {b.file}:{b.line}
      </p>
    </BlockShell>
  );
}

function CallMetricsView({
  block,
}: {
  block: ViewBlock & { body: { fan_in: number; fan_out: number } };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <dl className="grid grid-cols-2 gap-2 text-sm">
        <Stat label="Fan in" value={b.fan_in} />
        <Stat label="Fan out" value={b.fan_out} />
      </dl>
    </BlockShell>
  );
}

function SignatureView({
  block,
}: {
  block: ViewBlock & { body: { signature: string } };
}) {
  return (
    <BlockShell id={block.id} title={block.title}>
      <pre
        tabIndex={0}
        className="overflow-x-auto rounded-sm p-2 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code>{block.body.signature}</code>
      </pre>
    </BlockShell>
  );
}

interface CallListProps {
  block: ViewBlock & { body: CallListBlockBody };
  onSelectObject?: (id: string) => void;
}

function CallListView({ block, onSelectObject }: CallListProps) {
  const items = block.body.items;
  if (items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No items.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul
        data-testid={`view-block-${block.id}-items`}
        className="flex flex-col gap-0.5"
      >
        {items.map((item: RelationItem) => (
          <CallListItemRow
            key={item.object_id}
            item={item}
            onSelectObject={onSelectObject}
          />
        ))}
      </ul>
    </BlockShell>
  );
}

function CallListItemRow({
  item,
  onSelectObject,
}: {
  item: RelationItem;
  onSelectObject?: (id: string) => void;
}) {
  const interactive = Boolean(onSelectObject);
  // When interactive we render a <button> INSIDE the <li>. Putting
  // `role="button"` directly on an <li> breaks the list semantics
  // (axe: "List element has direct children that are not allowed").
  // The <li> stays a list item; the <button> carries the focusable
  // affordance.
  return (
    <li
      data-testid={`view-block-item-${item.object_id}`}
      className="list-none"
    >
      {interactive ? (
        <button
          type="button"
          onClick={() => onSelectObject?.(item.object_id)}
          data-testid={`view-block-item-button-${item.object_id}`}
          className="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left text-sm"
          style={{
            backgroundColor: "transparent",
            color: "var(--color-text-primary)",
          }}
        >
          <span
            aria-hidden="true"
            className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            ƒ
          </span>
          <span className="min-w-0 flex-1 truncate" title={item.name}>
            {item.name}
          </span>
          <span
            className="font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            {item.file}:{item.line}
          </span>
        </button>
      ) : (
        <div
          className="flex items-center gap-2 rounded-sm px-2 py-1 text-sm"
          style={{ color: "var(--color-text-primary)" }}
        >
          <span
            aria-hidden="true"
            className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            ƒ
          </span>
          <span className="min-w-0 flex-1 truncate" title={item.name}>
            {item.name}
          </span>
          <span
            className="font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            {item.file}:{item.line}
          </span>
        </div>
      )}
    </li>
  );
}

function SourceView({
  block,
}: {
  block: ViewBlock & { body: SourceSliceBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {b.file} · starting at line {b.line}
      </p>
      <ol
        className="mt-2 flex flex-col font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          borderRadius: "var(--radius-sm)",
          overflow: "hidden",
        }}
      >
        {b.lines.map((ln: SourceLine) => (
          <li
            key={ln.line}
            data-testid={`source-line-${ln.line}`}
            className="flex"
          >
            <span
              aria-hidden="true"
              className="select-none px-2 py-0.5 text-right"
              style={{
                width: "3.5rem",
                color: "var(--color-text-muted)",
                borderRight: "1px solid var(--color-border)",
              }}
            >
              {ln.line}
            </span>
            <span className="flex-1 whitespace-pre px-2 py-0.5">
              {ln.text || " "}
            </span>
          </li>
        ))}
      </ol>
    </BlockShell>
  );
}

function SymbolQualityIdentityView({
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

function FileQualityIdentityView({
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

function ScopeQualityIdentityView({
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

function QualityGateView({
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

function IssuesListView({
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

function IssueIdentityView({
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

function IssueLocationView({
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

function IssueMessageView({
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

function RuleIdentityView({
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

function FileIdentityView({
  block,
}: {
  block: ViewBlock & { body: FileIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {b.path}
      </p>
      <dl className="mt-1 grid grid-cols-2 gap-1 text-xs">
        <Stat label="Lines" value={b.line_count} small />
        <Stat label="Symbols" value={b.symbol_count} small />
      </dl>
    </BlockShell>
  );
}

function KindsView({
  block,
}: {
  block: ViewBlock & { body: KindsBreakdownBlockBody };
}) {
  const entries = Object.entries(block.body.breakdown);
  if (entries.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No symbols.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <dl className="grid grid-cols-2 gap-1 text-xs">
        {entries.map(([kind, count]) => (
          <div
            key={kind}
            className="flex items-center justify-between rounded-sm px-2 py-1"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            <dt style={{ color: "var(--color-text-secondary)" }}>{kind}</dt>
            <dd className="font-mono">{count}</dd>
          </div>
        ))}
      </dl>
    </BlockShell>
  );
}

function FileSymbolsView({
  block,
}: {
  block: ViewBlock & { body: FileSymbolsBlockBody };
}) {
  if (block.body.items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No symbols.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul className="flex flex-col gap-0.5 text-sm">
        {block.body.items.map((it) => (
          <li
            key={it.object_id}
            className="flex items-center gap-2 rounded-sm px-2 py-1"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            <span
              aria-hidden="true"
              className="font-mono text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              ƒ
            </span>
            <span className="min-w-0 flex-1 truncate" title={it.name}>
              {it.name}
            </span>
            <span
              className="font-mono text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              {it.kind} · {it.line}
            </span>
          </li>
        ))}
      </ul>
    </BlockShell>
  );
}

function ScopeIdentityView({
  block,
}: {
  block: ViewBlock & { body: ScopeIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {b.path}
      </p>
      <dl className="mt-1 grid grid-cols-2 gap-1 text-xs">
        <Stat label="Files" value={b.file_count} small />
        <Stat label="Symbols" value={b.symbol_count} small />
        <div
          className="col-span-2 flex items-center justify-between rounded-sm px-2 py-1"
          style={{ backgroundColor: "var(--color-surface-overlay)" }}
        >
          <dt style={{ color: "var(--color-text-secondary)" }}>
            Promotion ready
          </dt>
          <dd
            className="font-mono"
            style={{
              color: b.promotion_ready
                ? "var(--color-success)"
                : "var(--color-text-muted)",
            }}
          >
            {b.promotion_ready ? "yes" : "no"}
          </dd>
        </div>
      </dl>
    </BlockShell>
  );
}

function ScopeFilesView({
  block,
}: {
  block: ViewBlock & { body: ScopeFilesBlockBody };
}) {
  if (block.body.files.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No files.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul className="flex flex-col gap-0.5 text-xs">
        {block.body.files.map((f) => (
          <li
            key={f}
            className="rounded-sm px-2 py-1 font-mono"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            {f}
          </li>
        ))}
      </ul>
    </BlockShell>
  );
}

function CrossScopeView({
  block,
}: {
  block: ViewBlock & { body: CrossScopeBlockBody };
}) {
  if (block.body.entries.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>
          No cross-scope relations.
        </p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {block.body.scope}
      </p>
      <table className="mt-2 w-full text-xs">
        <thead style={{ color: "var(--color-text-muted)" }}>
          <tr className="text-left">
            <th className="px-2 py-1 font-medium">Scope</th>
            <th className="px-2 py-1 font-medium">Out</th>
            <th className="px-2 py-1 font-medium">In</th>
          </tr>
        </thead>
        <tbody>
          {block.body.entries.map((e) => (
            <tr
              key={e.scope}
              style={{ borderTop: "1px solid var(--color-border)" }}
            >
              <td
                className="px-2 py-1 font-mono"
                style={{ color: "var(--color-text-primary)" }}
              >
                {e.scope}
              </td>
              <td className="px-2 py-1 font-mono">{e.outgoing_count}</td>
              <td className="px-2 py-1 font-mono">{e.incoming_count}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </BlockShell>
  );
}

function HotspotsView({
  block,
  onSelectObject,
}: {
  block: ViewBlock & { body: HotspotsBlockBody };
  onSelectObject?: (id: string) => void;
}) {
  if (block.body.items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No hotspots.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ol className="flex flex-col gap-0.5 text-sm">
        {block.body.items.map((it, idx) => (
          <li
            key={it.object_id}
            data-testid={`view-block-hotspot-${it.object_id}`}
            className="list-none"
          >
            {onSelectObject ? (
              <button
                type="button"
                onClick={() => onSelectObject(it.object_id)}
                data-testid={`view-block-hotspot-button-${it.object_id}`}
                className="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left"
                style={{ backgroundColor: "var(--color-surface-overlay)" }}
              >
                <span
                  aria-hidden="true"
                  className="inline-flex h-5 w-5 flex-none items-center justify-center rounded-full font-mono text-xs"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  {idx + 1}
                </span>
                <span className="min-w-0 flex-1 truncate" title={it.name}>
                  {it.name}
                </span>
                <span
                  className="font-mono text-xs"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {it.file}:{it.line}
                </span>
              </button>
            ) : (
              <div
                className="flex items-center gap-2 rounded-sm px-2 py-1"
                style={{ backgroundColor: "var(--color-surface-overlay)" }}
              >
                <span
                  aria-hidden="true"
                  className="inline-flex h-5 w-5 flex-none items-center justify-center rounded-full font-mono text-xs"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  {idx + 1}
                </span>
                <span className="min-w-0 flex-1 truncate" title={it.name}>
                  {it.name}
                </span>
                <span
                  className="font-mono text-xs"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {it.file}:{it.line}
                </span>
              </div>
            )}
          </li>
        ))}
      </ol>
    </BlockShell>
  );
}

function UnknownBlockView({ block }: { block: UnknownViewBlock }) {
  return (
    <section
      data-testid="view-block-unknown"
      data-block-id={block.id}
      className="rounded-md p-3"
      style={{
        backgroundColor: "var(--color-surface-raised)",
        border: "1px dashed var(--color-warning)",
      }}
    >
      <header
        className="flex items-center justify-between gap-2"
        style={{ color: "var(--color-warning)" }}
      >
        <h3 className="text-xs font-semibold uppercase tracking-wide">
          {block.title}
        </h3>
        <span
          className="rounded-full px-2 py-0.5 font-mono text-xs"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-muted)",
          }}
        >
          unknown · {block.id}
        </span>
      </header>
      <p
        className="mt-1 text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        This block kind is not yet rendered natively. Showing raw JSON.
      </p>
      <pre
        tabIndex={0}
        className="mt-2 overflow-x-auto rounded-sm p-2 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code data-testid="view-block-unknown-json">
          {JSON.stringify(block.body, null, 2)}
        </code>
      </pre>
    </section>
  );
}

// ============================================================================
// Phase 10 — quality dashboard block renderers
// ============================================================================

/**
 * `quality_summary` — a compact card showing the scope's overall
 * quality rating, debt, total, and per-severity counts. Drives the
 * `QualityDashboard`'s summary header. The full interactive
 * dashboard (with click-to-filter) lives in `QualityDashboard.tsx`;
 * this renderer is the read-only view used inside the inspector.
 */
function QualitySummaryView({
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

/**
 * `quality_issue_detail` — one issue at full fidelity. The renderer
 * surfaces rule reference, location, message, and (when present)
 * the remediation tip from the backend. The `onSelectObject` path
 * lets the user jump from the issue to the underlying symbol by
 * clicking the location.
 */
function QualityIssueDetailView({
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

/** Severity color for the per-severity count cells in the summary. */
function severityTextColor(severity: string): string {
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

/** Trim an ISO timestamp to HH:MM:SS for compact display. */
function formatLastRun(iso: string): string {
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

function Stat({
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

function SeverityChip({ severity }: { severity: string }) {
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

// ============================================================================
// Blocks — render a list of blocks in a vertical stack
// ============================================================================

export interface BlocksProps {
  view: ContextualView;
  onSelectObject?: (objectId: string) => void;
}

/**
 * Render all blocks in a `ContextualView`. This is the convenience
 * wrapper used by the Inspector container — it iterates the
 * discriminated union and hands each block to `ViewBlock`.
 */
export function Blocks({ view, onSelectObject }: BlocksProps) {
  const items = useMemo(() => view.blocks, [view.blocks]);
  if (items.length === 0) {
    return (
      <div
        data-testid="view-blocks-empty"
        className="p-4 text-sm"
        style={{ color: "var(--color-text-muted)" }}
      >
        This view has no blocks yet.
      </div>
    );
  }
  return (
    <div
      data-testid="view-blocks"
      className="flex flex-col gap-2"
    >
      {items.map((block, idx) => (
        <ViewBlock
          key={`${(block as { id: string }).id}-${idx}`}
          block={block}
          onSelectObject={onSelectObject}
        />
      ))}
    </div>
  );
}
