/**
 * `useBootstrapWorkspace` — one-shot effect that auto-selects the first
 * workspace from `useWorkspaceList` if `state.workspace === null`.
 *
 * This handles the case where a user opens the Explorer without an
 * explicit workspace selection — we pick the first available workspace
 * so the landing page has something to render.
 */
import { useEffect } from "react";

import { useAppDispatch, useAppState } from "../state/context";
import { useWorkspaceList } from "./useWorkspace";

export function useBootstrapWorkspace() {
  const dispatch = useAppDispatch();
  const appState = useAppState();
  const { data: workspaces, isLoading } = useWorkspaceList();

  useEffect(() => {
    // Only bootstrap if we don't have a workspace yet
    if (appState.workspace !== null) return;
    // Wait for the workspace list to load
    if (isLoading) return;
    // Pick the first workspace if available
    const first = workspaces?.[0];
    if (first) {
      dispatch({ type: "SET_WORKSPACE", payload: first });
    }
  }, [appState.workspace, isLoading, workspaces, dispatch]);
}
