/**
 * `ScanBar` — scan button, progress bar, and graph stats for the active workspace.
 *
 * Shows in the Explorer top bar:
 * - "Scan" button (only when a workspace is open)
 * - Progress bar during active scan (stage: scan/extract/pg_upsert/done)
 * - Symbol/edge count after scan completes
 */
import { useState, type ReactNode } from "react";

import { useAppState } from "../state/context";
import { useJobStatus, useGraphStats, scanWorkspace } from "../hooks/useScanJob";
import type { JobStatus } from "../hooks/useScanJob";

export function ScanBar(): ReactNode {
  const { workspace } = useAppState();
  const workspaceId = workspace?.id ?? null;
  const [scanJobId, setScanJobId] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: job } = useJobStatus(scanJobId && scanning ? scanJobId : null);
  const { data: stats } = useGraphStats(workspaceId);

  const handleScan = async () => {
    if (!workspaceId || scanning) return;
    setScanning(true);
    setError(null);
    try {
      const accepted = await scanWorkspace(workspaceId);
      setScanJobId(accepted.job_id);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Scan failed");
      setScanning(false);
    }
  };

  // Auto-stop polling when job completes
  if (job?.status === "completed" || job?.status === "failed") {
    if (scanning) {
      // Small delay so the user sees "completed", then reset
      setTimeout(() => {
        setScanning(false);
        setScanJobId(null);
      }, 2000);
    }
  }

  const progressPct = job?.progress
    ? Math.round((job.progress.processed / Math.max(1, job.progress.total)) * 100)
    : 0;

  return (
    <div className="flex items-center gap-3" data-testid="scan-bar">
      {/* Scan button */}
      {workspaceId && (
        <button
          type="button"
          onClick={handleScan}
          disabled={scanning}
          className="rounded-md px-2 py-0.5 text-xs font-medium transition-colors"
          style={{
            backgroundColor: scanning
              ? "var(--color-surface-overlay)"
              : "var(--color-primary)",
            color: scanning
              ? "var(--color-text-muted)"
              : "var(--color-primary-foreground)",
            cursor: scanning ? "not-allowed" : "pointer",
          }}
        >
          {scanning ? "Scanning…" : "Scan"}
        </button>
      )}

      {/* Progress bar */}
      {scanning && job?.progress && (
        <div className="flex items-center gap-2" style={{ minWidth: 120 }}>
          <div
            className="h-1.5 rounded-full"
            style={{
              width: `${Math.max(5, progressPct)}%`,
              backgroundColor: "var(--color-primary)",
              transition: "width 0.3s ease",
              maxWidth: 150,
            }}
          />
          <span className="text-xs" style={{ color: "var(--color-text-secondary)" }}>
            {job.progress.stage} {job.progress.processed}/{job.progress.total}
          </span>
        </div>
      )}

      {/* Error */}
      {error && (
        <span className="text-xs" style={{ color: "var(--color-error)" }}>
          {error}
        </span>
      )}

      {/* Stats */}
      {stats && stats.symbol_count > 0 && (
        <span className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          {stats.symbol_count.toLocaleString()} symbols · {stats.edge_count.toLocaleString()} edges
        </span>
      )}

      {/* Completed notification */}
      {job?.status === "completed" && scanning && (
        <span className="text-xs" style={{ color: "var(--color-success)" }}>
          Scan complete! {job.result?.symbols ?? 0} symbols, {job.result?.edges ?? 0} edges in {(job.result?.duration_ms ?? 0) / 1000}s
        </span>
      )}
    </div>
  );
}
