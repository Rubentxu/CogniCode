/**
 * `TruncationBanner` — warning banner rendered above the
 * `ContextualPanel` content when the response carries
 * `truncated: true`. The message tells the user that the visible
 * set is a top-N slice and suggests tightening `max_nodes` or
 * focusing deeper.
 */
export interface TruncationBannerProps {
  /** Reason string from the response (e.g. "max_nodes_exceeded"). */
  reason?: string | null;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function TruncationBanner({ reason, className }: TruncationBannerProps) {
  const msg =
    reason === "max_nodes_exceeded"
      ? "Showing top N — refine with max_nodes or focus deeper"
      : "Showing top N — refine with max_nodes or focus deeper";
  return (
    <div
      data-testid="truncation-banner"
      role="status"
      aria-live="polite"
      className={className}
      style={{
        padding: "4px 8px",
        fontSize: 11,
        color: "#92400e",
        backgroundColor: "#fef3c7",
        border: "1px solid #fcd34d",
        borderRadius: 4,
        marginBottom: 6,
      }}
    >
      {msg}
    </div>
  );
}
