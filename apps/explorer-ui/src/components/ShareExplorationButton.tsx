/**
 * ShareExplorationButton — saves current navigation state and copies
 * a shareable URL to clipboard.
 *
 * Flow:
 * 1. Collect navigation state (panes + viewport)
 * 2. POST to /api/exploration-sessions (via saveExplorationSession — ADR-040 Wave 3)
 * 3. Copy URL with ?exploration=<session_id> to clipboard
 * 4. Show feedback: "Saving..." → "✓ Copied!" or "Failed"
 */
import { useState, useCallback } from "react";
import { useAppState } from "../state/context";
import { saveExplorationSession } from "../hooks/useExplorations";

type SaveStatus = "idle" | "saving" | "saved" | "failed";

export function ShareExplorationButton() {
  const appState = useAppState();
  const { navigation, workspace } = appState;
  const [status, setStatus] = useState<SaveStatus>("idle");

  const handleShare = useCallback(async () => {
    setStatus("saving");

    // Build exploration events from current state.
    // Each pane becomes an event (active pane first).
    const events = navigation.panes.map((pane) => ({
      object_id: pane.objectId,
      view_id: pane.activeViewId,
      query: null as string | null,
      ts: new Date().toISOString(),
    }));

    try {
      const session = await saveExplorationSession(
        workspace?.id ?? "default",
        events,
        navigation.panes,
      );

      const url = `${window.location.origin}${window.location.pathname}?exploration=${session.id}`;
      await navigator.clipboard.writeText(url);
      setStatus("saved");
      setTimeout(() => setStatus("idle"), 2000);
    } catch {
      // clipboard or network error
      setStatus("failed");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [navigation, workspace]);

  const label =
    status === "saving"
      ? "Saving..."
      : status === "saved"
        ? "✓ Copied!"
        : status === "failed"
          ? "Failed"
          : "Share";

  return (
    <button
      type="button"
      onClick={handleShare}
      aria-label="Share exploration"
      data-testid="share-exploration"
      className="rounded-md px-2 py-1 text-xs"
      style={{
        backgroundColor: "var(--color-surface-overlay)",
        color:
          status === "saved"
            ? "var(--color-primary)"
            : status === "failed"
              ? "var(--color-error, #dc2626)"
              : "var(--color-text-secondary)",
        border: "1px solid var(--color-border)",
      }}
    >
      {label}
    </button>
  );
}