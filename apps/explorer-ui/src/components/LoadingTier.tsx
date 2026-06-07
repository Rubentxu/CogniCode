/**
 * LoadingTier — three-tier loading state primitive.
 *
 * From auto-grill Q007-P2:
 *   1. `skeleton`  — render immediately (SSR / cached / first paint)
 *   2. `spinner`   — fallback if skeleton data isn't ready
 *   3. `empty`     — render the empty-state copy when data is `null`/`undefined`
 *
 * Use case: SWR returns `{ data, error, isLoading, isValidating }`. Wrap
 * the `children` (real content) in this primitive and pass the flags.
 */
import type { ReactNode } from "react";

export type LoadingTierVariant = "skeleton" | "spinner" | "empty" | "ready";

export interface LoadingTierProps {
  /**
   * The actual data to render. When `undefined` AND not in cache,
   * the component shows the skeleton/spinner. When `null`, the empty
   * state is shown (e.g., 404 — symbol not found). The typed shape
   * of the data is intentionally `unknown` so callers can pass
   * arrays / objects / primitives without coercing them to
   * `ReactNode` first.
   */
  data: unknown;
  /** True while SWR is fetching AND no cached data exists. */
  isLoading: boolean;
  /** True while SWR is revalidating in the background. */
  isValidating?: boolean;
  /** Optional error from SWR — surfaces a compact error state. */
  error?: Error | null;
  /** Label for screen readers (e.g., "Miller Columns loading"). */
  label: string;
  /** Optional skeleton fallback. Defaults to a 3-line pulse. */
  skeleton?: ReactNode;
  /** Optional empty-state copy. */
  emptyMessage?: ReactNode;
  /** Render this when data is ready. */
  children: ReactNode;
}

export function LoadingTier({
  data,
  isLoading,
  isValidating = false,
  error = null,
  label,
  skeleton,
  emptyMessage,
  children,
}: LoadingTierProps) {
  // Error tier — show inline, not full-page takeover.
  if (error !== null) {
    return (
      <div
        role="alert"
        aria-label={`${label} error`}
        className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-sm"
        style={{ color: "var(--color-error)" }}
      >
        <span className="font-semibold">Failed to load {label}</span>
        <span style={{ color: "var(--color-text-secondary)" }}>
          {error.message}
        </span>
      </div>
    );
  }

  // Tier 1: Skeleton (cache hit but no data yet, OR first paint with cached layout)
  if (isLoading && data === undefined) {
    return (
      <div
        role="status"
        aria-live="polite"
        aria-busy="true"
        aria-label={`Loading ${label}`}
        className="flex h-full w-full flex-col gap-2 p-3"
      >
        {skeleton ?? <DefaultSkeleton />}
        <span className="sr-only">Loading {label}…</span>
      </div>
    );
  }

  // Tier 3: Empty state (data resolved to null — e.g., symbol not found)
  if (data === null || data === undefined) {
    return (
      <div
        role="status"
        aria-label={`${label} empty`}
        className="flex h-full w-full items-center justify-center p-4 text-center text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {emptyMessage ?? `No ${label.toLowerCase()} to show.`}
      </div>
    );
  }

  // Tier 4: Ready — render content. Subtle revalidation indicator.
  return (
    <div
      aria-busy={isValidating}
      aria-label={label}
      className="relative h-full w-full"
    >
      {children}
      {isValidating && (
        <div
          aria-hidden="true"
          className="absolute right-2 top-2 h-2 w-2 animate-pulse rounded-full"
          style={{ backgroundColor: "var(--color-primary)" }}
        />
      )}
    </div>
  );
}

function DefaultSkeleton() {
  return (
    <>
      <div
        className="h-4 w-3/4 animate-pulse rounded"
        style={{ backgroundColor: "var(--color-surface-overlay)" }}
      />
      <div
        className="h-4 w-1/2 animate-pulse rounded"
        style={{ backgroundColor: "var(--color-surface-overlay)" }}
      />
      <div
        className="h-4 w-2/3 animate-pulse rounded"
        style={{ backgroundColor: "var(--color-surface-overlay)" }}
      />
    </>
  );
}
