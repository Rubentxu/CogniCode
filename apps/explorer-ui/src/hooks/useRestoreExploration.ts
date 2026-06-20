/**
 * useRestoreExploration — on mount, parses ?exploration=<id> from URL
 * and restores the exploration session including pane state and viewport
 * (ADR-040 Wave 3 H3 fix — was previously ignoring panes field).
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

        // Restore each pane snapshot: open pane + apply viewport state
        // (preserves zoom/pan from the saved exploration — was previously lost).
        for (const paneSnapshot of session.panes) {
          dispatch({
            type: "PUSH_PANE",
            payload: {
              objectId: paneSnapshot.object_id,
              viewId: paneSnapshot.view_id,
            },
          });
          if (paneSnapshot.viewport) {
            dispatch({
              type: "UPDATE_PANE_VIEWPORT",
              payload: {
                paneId: `pane-${paneSnapshot.pane_id}`,
                viewport: paneSnapshot.viewport,
              },
            });
          }
        }
      })
      .catch(() => {
        // Session not found, network error, or schema validation failed — silently skip.
      });
  }, [dispatch]);
}