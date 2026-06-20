/**
 * ViewBlocks — domain-split block renderers.
 *
 * Exported for use by ViewBlock.tsx. Each module contains one logical
 * group of related views.
 */
export { BlockShell, SeverityChip, Stat, formatLastRun, ratingColor, severityColor, severityTextColor } from "./shared";
export { typed, isKnownBlockId, type CallListProps, type BlockShellProps } from "./types";

export { IdentityView } from "./identity";
export { CallListView, CallMetricsView, SignatureView, SourceView } from "./call";
export { FileIdentityView, FileSymbolsView, KindsView } from "./file";
export {
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
} from "./quality";
export { CrossScopeView, ScopeFilesView, ScopeIdentityView } from "./scope";
export { HotspotsView } from "./hotspots";
export { UnknownBlockView } from "./unknown";
