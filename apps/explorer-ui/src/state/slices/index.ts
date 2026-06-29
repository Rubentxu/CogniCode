/**
 * Reducer slices — domain-based decomposition of appReducer.
 *
 * Each slice handles its own actions and ignores others (returning
 * state unchanged). RESET is handled by every slice to reset itself
 * to its initial value.
 *
 * The rootReducer composes all slices into the full AppState shape.
 */
import type { AppState } from "../context";
import { navigationReducer } from "./navigation";
import { spotterReducer, initialSpotterState } from "./spotter";
import { workspaceReducer } from "./workspace";
import { perspectiveReducer } from "./perspective";
import { lensSidebarReducer } from "./lensSidebar";
import { viewSpecWizardReducer } from "./viewSpecWizard";
import { landingWorkbenchReducer } from "./landingWorkbench";
import { c4OverlayReducer, initialC4OverlayState, type C4OverlayAction } from "./c4OverlaySlice";

export type RootReducer = (state: AppState, action: import("../context").Action) => AppState;

export function rootReducer(state: AppState, action: import("../context").Action): AppState {
  const navSlice = navigationReducer(
    {
      navigation: state.navigation,
      activeObjectId: state.activeObjectId,
      activeViewId: state.activeViewId,
      activeLensId: state.activeLensId,
      activeView: state.activeView,
    },
    action
  );

  const spotter = spotterReducer(
    { open: state.spotterOpen, kind: state.spotterKind ?? null },
    action
  );

  return {
    workspace: workspaceReducer(state.workspace, action),
    navigation: navSlice.navigation,
    activeObjectId: navSlice.activeObjectId,
    activeViewId: navSlice.activeViewId,
    activeLensId: navSlice.activeLensId,
    spotterOpen: spotter.open,
    spotterKind: spotter.kind,
    activeView: navSlice.activeView,
    perspective: perspectiveReducer(state.perspective, action),
    lensSidebar: lensSidebarReducer(state.lensSidebar, action as never),
    viewSpecWizard: viewSpecWizardReducer(state.viewSpecWizard, action as never),
    landingWorkbench: landingWorkbenchReducer(state.landingWorkbench, action),
    c4Overlay: c4OverlayReducer(state.c4Overlay, action as C4OverlayAction),
  };
}

// Re-export slice types for consumers
export type { NavigationSliceAction } from "./navigation";
export type { SpotterAction } from "./spotter";
export type { WorkspaceAction } from "./workspace";
export type { PerspectiveAction } from "./perspective";
export type { LensSidebarAction, LensSidebarState } from "./lensSidebar";
export type { LandingWorkbenchAction, LandingTabId } from "./landingWorkbench";
export type {
  ViewSpecWizardAction,
  ViewSpecWizardState,
} from "./viewSpecWizard";
export type { C4OverlayAction } from "./c4OverlaySlice";
