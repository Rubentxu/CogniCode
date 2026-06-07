/**
 * `HealthProbe` — top-bar connection indicator.
 *
 * Renders a small status chip with a coloured dot. When the backend
 * is unreachable, it shows a full-pane "connection screen" with a
 * Retry button so the user can recover without reloading the tab.
 *
 * The chip is just visual feedback during normal operation; the
 * full-screen takeover only fires when the probe reports an error
 * AND no cached successful response exists.
 */
import { useHealth } from "../hooks/useHealth";

export interface HealthProbeProps {
  /** Show the full-screen connection error UI when offline. Default true. */
  showFullScreenOnError?: boolean;
}

export function HealthProbe({ showFullScreenOnError = true }: HealthProbeProps) {
  const { status, data, error, isOnline, refresh } = useHealth();

  // -----------------------------------------------------------------------
  // Chip (always rendered, lives in the top bar)
  // -----------------------------------------------------------------------
  const chip = (
    <div
      data-testid="health-chip"
      data-status={status}
      role="status"
      aria-live="polite"
      aria-label={
        status === "online"
          ? `Connected to ${data?.service ?? "backend"}`
          : status === "offline"
            ? "Disconnected from backend"
            : "Checking backend status"
      }
      className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium"
      style={{
        backgroundColor: "var(--color-surface-overlay)",
        color: "var(--color-text-secondary)",
        border: "1px solid var(--color-border)",
      }}
    >
      <span
        aria-hidden="true"
        className="inline-block h-1.5 w-1.5 rounded-full"
        style={{
          backgroundColor:
            status === "online"
              ? "var(--color-success)"
              : status === "offline"
                ? "var(--color-error)"
                : "var(--color-warning)",
        }}
      />
      <span>
        {status === "online"
          ? data?.service ?? "online"
          : status === "offline"
            ? "offline"
            : "checking…"}
      </span>
    </div>
  );

  if (isOnline || !showFullScreenOnError) {
    return chip;
  }

  // -----------------------------------------------------------------------
  // Full-screen takeover when offline and the caller wants it
  // -----------------------------------------------------------------------
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="health-offline-title"
      data-testid="health-offline"
      className="flex h-full w-full flex-col items-center justify-center gap-4 p-6 text-center"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <div
        className="rounded-md px-2 py-1"
        style={{ backgroundColor: "var(--color-surface-overlay)" }}
      >
        {chip}
      </div>
      <h2
        id="health-offline-title"
        className="text-lg font-semibold"
        style={{ color: "var(--color-text-primary)" }}
      >
        Cannot reach the CogniCode Explorer backend
      </h2>
      <p
        className="max-w-md text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        The backend at <code>/api</code> did not respond. Make sure the
        axum service is running and try again.
      </p>
      {error && (
        <p
          className="max-w-md text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          {error.message}
        </p>
      )}
      <button
        type="button"
        onClick={() => void refresh()}
        className="rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
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
