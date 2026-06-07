/**
 * Per-panel Error Boundary for the CogniCode Explorer.
 *
 * Why a custom boundary (not `react-error-boundary`)?
 * - The auto-grill decision Q008-P2 committed to per-panel isolation: a crash
 *   in the Object Inspector must NOT take down the Miller Columns.
 * - We render a minimal fallback with a "Retry" action that calls the
 *   provided `onReset` (typically: close the panel, refetch, or navigate).
 * - Lightweight (~40 LOC) vs the 12KB react-error-boundary dep.
 *
 * React 19 note: `componentDidCatch` and `getDerivedStateFromError` are still
 * the only way to do class-based error boundaries; no hook equivalent yet.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";

export interface ErrorBoundaryProps {
  /** Optional fallback override (defaults to <DefaultErrorFallback />). */
  fallback?: (error: Error, reset: () => void) => ReactNode;
  /** Called when the user clicks "Retry" in the default fallback. */
  onReset?: () => void;
  /** Human-readable label for the boundary (panel name, etc). */
  label?: string;
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // Surfacing to the console so dev tooling (Chrome DevTools, Sentry, etc.)
    // can pick it up. Avoid swallowing the error in production.
    console.error(`[ErrorBoundary:${this.props.label ?? "root"}]`, error, info);
  }

  reset = (): void => {
    this.setState({ error: null });
    this.props.onReset?.();
  };

  override render(): ReactNode {
    const { error } = this.state;
    if (error === null) {
      return this.props.children;
    }
    if (this.props.fallback) {
      return this.props.fallback(error, this.reset);
    }
    return <DefaultErrorFallback error={error} onReset={this.reset} label={this.props.label} />;
  }
}

export interface ErrorFallbackProps {
  error: Error;
  onReset: () => void;
  label?: string;
}

/**
 * Default fallback UI — dark-themed, accessible, minimal copy.
 * Uses semantic tokens from `tailwind.css` (`@theme`).
 */
export function DefaultErrorFallback({ error, onReset, label }: ErrorFallbackProps) {
  return (
    <div
      role="alert"
      aria-live="assertive"
      className="flex h-full w-full flex-col items-center justify-center gap-3 p-6 text-center"
      style={{ backgroundColor: "var(--color-surface-raised)" }}
    >
      <h2
        className="text-base font-semibold"
        style={{ color: "var(--color-error)" }}
      >
        {label ? `${label} crashed` : "Something went wrong"}
      </h2>
      <p
        className="max-w-md text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {error.message || "An unexpected error occurred."}
      </p>
      <button
        type="button"
        onClick={onReset}
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
