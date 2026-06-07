/**
 * CogniCode Explorer — root App component.
 *
 * Phase 4+5: wires the AppProvider (Context + useReducer) and the
 * 3-panel Shell together. The Shell reads its state from the context
 * and dispatches actions from inside its child panels.
 */
import { useEffect, useState } from "react";
import { SWRConfig } from "swr";

import { ApiError, getApiBaseUrl } from "./api/client";
import { healthResponseSchema } from "./api/schemas";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Shell } from "./components/Shell";
import { useAppReducer, AppContext } from "./state/context";

export default function App() {
  const { state, dispatch } = useAppReducer();

  return (
    <SWRConfig
      value={{
        revalidateOnFocus: false,
        shouldRetryOnError: true,
        errorRetryCount: 2,
      }}
    >
      <AppContext.Provider value={{ state, dispatch }}>
        <ErrorBoundary label="Explorer">
          <ConnectionGate />
        </ErrorBoundary>
      </AppContext.Provider>
    </SWRConfig>
  );
}

// ============================================================================
// Connection gate — single-shot health probe, then mount the Shell
// ============================================================================

type GateState = { isOnline: boolean | null; error: Error | null };

function ConnectionGate() {
  const [{ isOnline, error }, setState] = useState<GateState>({
    isOnline: null,
    error: null,
  });

  const refresh = async () => {
    try {
      const base = getApiBaseUrl();
      const response = await fetch(`${base.replace(/\/$/, "")}/health`);
      if (!response.ok) {
        throw new ApiError({
          message: `Health probe failed: ${response.status} ${response.statusText}`,
          status: response.status,
          url: response.url,
        });
      }
      const raw = await response.json();
      healthResponseSchema.parse(raw);
      setState({ isOnline: true, error: null });
    } catch (e) {
      setState({
        isOnline: false,
        error: e instanceof Error ? e : new Error(String(e)),
      });
    }
  };

  useEffect(() => {
    // The probe resolves asynchronously and the handler will call
    // setState once the response lands. We intentionally do NOT poll
    // — the Shell's top-bar HealthProbe takes over the connection-
    // status UI once mounted.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refresh();
  }, []);

  if (isOnline === null) {
    return (
      <div
        role="status"
        aria-live="polite"
        aria-label="Checking backend status"
        data-testid="connection-gate-checking"
        className="flex h-full w-full items-center justify-center"
        style={{
          backgroundColor: "var(--color-surface)",
          color: "var(--color-text-secondary)",
        }}
      >
        <span className="text-sm">Checking backend status…</span>
      </div>
    );
  }

  if (!isOnline) {
    return (
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="connection-gate-title"
        data-testid="connection-gate-offline"
        className="flex h-full w-full flex-col items-center justify-center gap-4 p-6 text-center"
        style={{ backgroundColor: "var(--color-surface)" }}
      >
        <h2
          id="connection-gate-title"
          className="text-lg font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Cannot reach the CogniCode Explorer backend
        </h2>
        <p
          className="max-w-md text-sm"
          style={{ color: "var(--color-text-secondary)" }}
        >
          The backend at <code>/api</code> did not respond. Make sure
          the axum service is running and try again.
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

  return <Shell />;
}
