/**
 * Navigation slice — pane stack state management.
 *
 * Public API: types, reducer, and slice are re-exported from the
 * dedicated sub-modules. Consumers should import from this barrel:
 *
 *   import { Pane, paneStackReducer } from "../slices/navigation";
 */
export * from "./navigation/types";
export * from "./navigation/reducer";
export * from "./navigation/slice";
