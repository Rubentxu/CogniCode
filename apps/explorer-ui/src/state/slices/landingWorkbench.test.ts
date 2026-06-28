/**
 * Unit tests for the landingWorkbench slice reducer.
 *
 * Tests cover SET_LANDING_TAB, RESET, and unknown-action passthrough.
 */
import { describe, it, expect } from "vitest";
import {
  landingWorkbenchReducer,
  initialLandingWorkbenchState,
} from "./landingWorkbench";

describe("landingWorkbenchReducer", () => {
  it("defaults to start tab", () => {
    expect(initialLandingWorkbenchState.activeTab).toBe("start");
  });

  it("handles SET_LANDING_TAB", () => {
    const next = landingWorkbenchReducer(initialLandingWorkbenchState, {
      type: "SET_LANDING_TAB",
      payload: { tab: "investigations" },
    });
    expect(next.activeTab).toBe("investigations");
  });

  it("handles SET_LANDING_TAB to resume tab", () => {
    const next = landingWorkbenchReducer(initialLandingWorkbenchState, {
      type: "SET_LANDING_TAB",
      payload: { tab: "resume" },
    });
    expect(next.activeTab).toBe("resume");
  });

  it("handles SET_LANDING_TAB to graph tab", () => {
    const next = landingWorkbenchReducer(initialLandingWorkbenchState, {
      type: "SET_LANDING_TAB",
      payload: { tab: "graph" },
    });
    expect(next.activeTab).toBe("graph");
  });

  it("resets to start tab on RESET", () => {
    const dirty = { activeTab: "graph" as const };
    const next = landingWorkbenchReducer(dirty, { type: "RESET" });
    expect(next).toEqual(initialLandingWorkbenchState);
    expect(next.activeTab).toBe("start");
  });

  it("ignores unknown actions", () => {
    const next = landingWorkbenchReducer(initialLandingWorkbenchState, {
      type: "TOGGLE_SPOTTER",
    });
    expect(next).toBe(initialLandingWorkbenchState);
  });

  it("preserves other state fields on SET_LANDING_TAB", () => {
    // The slice only manages activeTab — other fields are not touched
    const next = landingWorkbenchReducer(initialLandingWorkbenchState, {
      type: "SET_LANDING_TAB",
      payload: { tab: "resume" },
    });
    expect(Object.keys(next)).toEqual(["activeTab"]);
  });
});
