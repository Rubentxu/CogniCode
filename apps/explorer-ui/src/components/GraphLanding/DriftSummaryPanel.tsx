/**
 * `DriftSummaryPanel` — shows drift findings that don't match any visible C4 node.
 *
 * Rendered below the graph canvas when:
 * - perspective === "c4" && driftEnabled && driftReport exists
 * - some findings reference containers not present in the visible node set
 */
import type { DriftReport } from "../../api/types";
import { normalizeContainerName } from "./GraphLanding";

interface DriftSummaryPanelProps {
  driftReport: DriftReport;
  /** Labels of currently visible C4 nodes */
  c4NodeLabels: string[];
}

const KIND_STYLES: Record<string, string> = {
  missing: "bg-red-500",
  extra: "bg-orange-500",
  wrong_sub_kind: "bg-yellow-500",
  boundary_violation: "bg-blue-500",
};

function cn(...classes: (string | false | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

export function DriftSummaryPanel({ driftReport, c4NodeLabels }: DriftSummaryPanelProps) {
  const normalizedLabels = new Set(c4NodeLabels.map(normalizeContainerName));

  const unmatched = driftReport.findings.filter((f) => {
    const name = normalizeContainerName(f.actual !== "—" ? f.actual : f.expected);
    return !normalizedLabels.has(name);
  });

  if (unmatched.length === 0) return null;

  return (
    <div
      data-testid="c4-drift-summary"
      className="border border-red-200 bg-red-50 rounded p-3 mt-2"
    >
      <h4 className="text-sm font-semibold text-red-700 mb-2">
        Drift Findings ({unmatched.length})
      </h4>
      <div className="space-y-1">
        {unmatched.map((finding, i) => (
          <div key={i} className="flex items-center gap-2 text-xs">
            <span className="font-mono text-gray-700">
              {finding.actual !== "—" ? finding.actual : finding.expected}
            </span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-white text-xs",
                KIND_STYLES[finding.kind],
              )}
            >
              {finding.kind.replaceAll("_", " ")}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
