/**
 * Shared types and helpers for ViewBlock renderers.
 */
import type {
  CallListBlockBody,
  ViewBlock,
} from "../../../api/types";

// ============================================================================
// Helpers — exported so ViewBlock.tsx can use them
// ============================================================================

/**
 * Cast a `ViewBlock | UnknownViewBlock` to one of the typed shapes
 * keyed by `id`. We re-narrow with a discriminant check first to
 * keep the rest of the file sound. The `unknown` intermediate is
 * necessary because TS cannot correlate `id` to the body shape
 * automatically when the block itself is widened.
 */
export function typed<B extends { id: string }, K extends ViewBlock["id"]>(
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

export function isKnownBlockId(id: string): id is ViewBlock["id"] {
  return KNOWN_IDS.has(id);
}

// ============================================================================
// Shared interfaces
// ============================================================================

export interface CallListProps {
  block: ViewBlock & { body: CallListBlockBody };
  onSelectObject?: (id: string) => void;
}

export interface BlockShellProps {
  id: string;
  title: string;
  children: React.ReactNode;
  testId?: string;
}
