/**
 * `ShellBootstrap` — all side effects for the Shell.
 *
 * Handles:
 * - Exploration restore from ?exploration=<id> on mount (ADR-016 Fase 4)
 * - Snapshot cache to localStorage on pane changes (ADR-040 Wave 3)
 * - Workspace list fetch + auto-select first workspace
 *
 * NO layout — purely provides state via the children render prop.
 */
import { useEffect, type ReactNode } from "react";

import { useAppDispatch, useAppState } from "../state/context";
import { useRestoreExploration } from "../hooks/useRestoreExploration";
import { useSnapshotCache } from "../hooks/useExplorations";
import { useWorkspaceList } from "../hooks/useWorkspace";
import type { WorkspaceSummary } from "../api/types";

export interface ShellBootstrapProps {
  children: (state: { workspace: WorkspaceSummary | null }) => ReactNode;
}

export function ShellBootstrap({ children }: ShellBootstrapProps) {
  const dispatch = useAppDispatch();
  const appState = useAppState();

  // Restore exploration from ?exploration=<id> on mount (ADR-016 Fase 4).
  useRestoreExploration();

  // Cache exploration snapshot to localStorage on every pane change (ADR-040 Wave 3).
  // Wired here so any change (pan/zoom, drill, close) is persisted before the
  // user navigates away. The sessionId is a fixed "current" for single-session
  // cache per workspace.
  const sessionId = "current";
  const workspaceId = appState.workspace?.id ?? null;
  useSnapshotCache(workspaceId, sessionId, appState.navigation.panes);

  // Bootstrap workspace — auto-select first workspace from list so GraphLanding
  // renders (without this, workspace stays null).
  const { data: workspaceList } = useWorkspaceList();
  useEffect(() => {
    if (!appState.workspace && workspaceList?.[0]) {
      dispatch({ type: "SET_WORKSPACE", payload: workspaceList[0] });
    }
  }, [workspaceList, appState.workspace, dispatch]);

  return children({ workspace: appState.workspace });
}
