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
          navigation_mode: string;
          events: { object_id: string; view_id: string | null }[];
        }>;
      })
      .then((session) => {
        // In column mode, restore each event as a PUSH_COLUMN.
        // In pane-stack mode, restore each event as a PUSH_PANE.
        const actions = session.events.map((ev) => {
          if (session.navigation_mode === "pane-stack") {
            return {
              type: "PUSH_PANE" as const,
              payload: { objectId: ev.object_id, viewId: ev.view_id ?? undefined },
            };
          }
          return {
            type: "PUSH_COLUMN" as const,
            payload: {
              object_id: ev.object_id,
              active_view: ev.view_id,
            },
          };
        });

        // Switch to the session's mode first, then replay actions.
        dispatch({
          type: "SET_NAVIGATION_MODE",
          payload: { mode: session.navigation_mode as "column" | "pane-stack" },
        });
        for (const action of actions) {
          dispatch(action as any);
        }
      })
      .catch(() => {
        // Session not found or network error — silently skip.
      });
  }, [dispatch]);
}
