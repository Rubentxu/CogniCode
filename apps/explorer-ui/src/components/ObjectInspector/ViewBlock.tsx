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
  ContextualView,
  ViewBlock,
  UnknownViewBlock,
} from "../../api/types";

import {
  typed,
  isKnownBlockId,
  // Identity
  IdentityView,
  // Call
  CallListView,
  CallMetricsView,
  SignatureView,
  SourceView,
  // File
  FileIdentityView,
  FileSymbolsView,
  KindsView,
  // Quality
  FileQualityIdentityView,
  QualityGateView,
  QualityIssueDetailView,
  QualitySummaryView,
  IssueIdentityView,
  IssueLocationView,
  IssueMessageView,
  IssuesListView,
  RuleIdentityView,
  ScopeQualityIdentityView,
  SymbolQualityIdentityView,
  // Scope
  CrossScopeView,
  ScopeFilesView,
  ScopeIdentityView,
  // Hotspots
  HotspotsView,
  // Unknown
  UnknownBlockView,
} from "./ViewBlocks";

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
  const known = isKnownBlockId(id);
  if (!known) {
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
