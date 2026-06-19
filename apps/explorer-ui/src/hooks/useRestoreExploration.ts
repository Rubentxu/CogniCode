/**
 * useRestoreExploration — on mount, parses ?exploration=<id> from URL
 * and restores the exploration session.
 */
import { useEffect } from "react";
import { useAppDispatch } from "../state/context";

export function useRestoreExploration() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const sessionId = params.get("exploration");
    if (!sessionId) return;

    fetch(`/api/exploration-sessions/${sessionId}`)
      .then((r) => {
        if (!r.ok) throw new Error("not found");
        return r.json() as Promise<{
          events: { object_id: string; view_id: string | null }[];
        }>;
      })
      .then((session) => {
        // Restore each event as a PUSH_PANE.
        for (const ev of session.events) {
          dispatch({
            type: "PUSH_PANE",
            payload: { objectId: ev.object_id, viewId: ev.view_id ?? undefined },
          });
        }
      })
      .catch(() => {
        // Session not found or network error — silently skip.
      });
  }, [dispatch]);
}
