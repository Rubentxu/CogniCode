/**
 * useRestoreExploration — on mount, parses ?exploration=<id> from URL
 * and restores the exploration session including pane state, viewport, and notes.
 * Uses RESTORE_PANE to atomically restore each pane with its snapshot + note.
 */
import { useEffect } from "react";
import { useAppDispatch } from "../state/context";
import { explorationSessionSchema } from "../api/schemas";

export function useRestoreExploration() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const sessionId = params.get("exploration");
    if (!sessionId) return;

    fetch(`/api/exploration-sessions/${sessionId}`)
      .then((r) => {
        if (!r.ok) throw new Error("not found");
        return r.json();
      })
      .then((raw) => {
        // Validate response against Zod schema (defense-in-depth)
        const session = explorationSessionSchema.parse(raw);

        // Restore each pane atomically: RESTORE_PANE uses the snapshot's pane_id
        // to construct the deterministic pane ID and applies viewport + note in one step.
        for (const paneSnapshot of session.panes) {
          dispatch({
            type: "RESTORE_PANE",
            payload: {
              paneSnapshot,
              // note is intentionally NOT sent to server (ADR-005 client-only),
              // but is restored from the localStorage snapshot (see useSnapshotCache).
              // During URL-based restore, we don't have the note — it will be
              // present when restoring from the localStorage cache.
              note: undefined,
            },
          });
        }
      })
      .catch(() => {
        // Session not found, network error, or schema validation failed — silently skip.
      });
  }, [dispatch]);
}
