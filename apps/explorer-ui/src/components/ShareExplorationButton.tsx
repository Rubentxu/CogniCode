/**
 * ShareExplorationButton — saves current navigation state and copies
 * a shareable URL to clipboard.
 *
 * Flow:
 * 1. Collect navigation state (panes + chain in column mode)
 * 2. POST to /api/exploration-sessions
 * 3. Copy URL with ?exploration=<session_id> to clipboard
 */
import { useState, useCallback } from "react";
import { useAppState } from "../state/context";
import type { Pane } from "../state/navigation";

export function ShareExplorationButton() {
  const { navigation } = useAppState();
  const [copied, setCopied] = useState(false);

  const handleShare = useCallback(async () => {
    const mode = navigation.mode;

    // Build exploration events from current state.
    // In column mode: treat the chain as a linear sequence.
    // In pane-stack mode: treat each pane as an event (active pane first).
    const events = mode === "column"
      ? navigation.chain.map((col) => ({
          object_id: col.object_id,
          view_id: col.active_view,
          query: null as string | null,
          ts: new Date().toISOString(),
        }))
      : navigation.panes.map((pane: Pane) => ({
          object_id: pane.objectId,
          view_id: pane.activeViewId,
          query: null as string | null,
          ts: new Date().toISOString(),
        }));

    try {
      const resp = await fetch("/api/exploration-sessions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          workspace_id: "default",
          events,
          navigation_mode: mode,
        }),
      });
      if (!resp.ok) return;
      const session: { id: string } = await resp.json();
      const url = `${window.location.origin}${window.location.pathname}?exploration=${session.id}`;
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard or network error — silently fail
    }
  }, [navigation]);

  return (
    <button
      type="button"
      onClick={handleShare}
      aria-label="Share exploration"
      data-testid="share-exploration"
      className="rounded-md px-2 py-1 text-xs"
      style={{
        backgroundColor: "var(--color-surface-overlay)",
        color: copied ? "var(--color-primary)" : "var(--color-text-secondary)",
        border: "1px solid var(--color-border)",
      }}
    >
      {copied ? "✓ Copied!" : "Share"}
    </button>
  );
}
