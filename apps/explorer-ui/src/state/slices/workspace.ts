/**
 * Workspace slice — the active workspace summary.
 *
 * Handles: SET_WORKSPACE, RESET
 */
import type { Action } from "../context";
import type { WorkspaceSummary } from "../../api/types";

export type WorkspaceAction = Extract<
  Action,
  { type: "SET_WORKSPACE" } | { type: "RESET" }
>;

export function workspaceReducer(
  state: WorkspaceSummary | null,
  action: Action
): WorkspaceSummary | null {
  switch (action.type) {
    case "SET_WORKSPACE":
      return action.payload;
    case "RESET":
      return null;
    default:
      return state;
  }
}
