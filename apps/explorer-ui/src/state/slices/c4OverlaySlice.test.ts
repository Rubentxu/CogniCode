/**
 * Tests for `c4OverlaySlice` reducer.
 *
 * Validates:
 * - toggleDrift action
 * - toggleHotspots action
 * - Independent toggles (one does not affect the other)
 */
import { describe, expect, it } from "vitest";

import {
  c4OverlayReducer,
  initialC4OverlayState,
  type C4OverlayAction,
} from "./c4OverlaySlice";

describe("c4OverlaySlice", () => {
  describe("toggleDrift", () => {
    it("toggles driftEnabled from false to true", () => {
      const action: C4OverlayAction = { type: "c4-overlay/toggleDrift" };
      const result = c4OverlayReducer(initialC4OverlayState, action);
      expect(result.driftEnabled).toBe(true);
      expect(result.hotspotsEnabled).toBe(false);
    });

    it("toggles driftEnabled from true to false", () => {
      const action: C4OverlayAction = { type: "c4-overlay/toggleDrift" };
      const state = { ...initialC4OverlayState, driftEnabled: true };
      const result = c4OverlayReducer(state, action);
      expect(result.driftEnabled).toBe(false);
    });
  });

  describe("toggleHotspots", () => {
    it("toggles hotspotsEnabled from false to true", () => {
      const action: C4OverlayAction = { type: "c4-overlay/toggleHotspots" };
      const result = c4OverlayReducer(initialC4OverlayState, action);
      expect(result.hotspotsEnabled).toBe(true);
      expect(result.driftEnabled).toBe(false);
    });

    it("toggles hotspotsEnabled from true to false", () => {
      const action: C4OverlayAction = { type: "c4-overlay/toggleHotspots" };
      const state = { ...initialC4OverlayState, hotspotsEnabled: true };
      const result = c4OverlayReducer(state, action);
      expect(result.hotspotsEnabled).toBe(false);
    });
  });

  describe("independent toggles", () => {
    it("toggling drift does not affect hotspots", () => {
      const state = { ...initialC4OverlayState, hotspotsEnabled: true };
      const action: C4OverlayAction = { type: "c4-overlay/toggleDrift" };
      const result = c4OverlayReducer(state, action);
      expect(result.driftEnabled).toBe(true);
      expect(result.hotspotsEnabled).toBe(true); // unchanged
    });

    it("toggling hotspots does not affect drift", () => {
      const state = { ...initialC4OverlayState, driftEnabled: true };
      const action: C4OverlayAction = { type: "c4-overlay/toggleHotspots" };
      const result = c4OverlayReducer(state, action);
      expect(result.driftEnabled).toBe(true); // unchanged
      expect(result.hotspotsEnabled).toBe(true);
    });
  });

  describe("unknown action", () => {
    it("returns state unchanged", () => {
      const action = { type: "unknown/action" } as unknown as C4OverlayAction;
      const result = c4OverlayReducer(initialC4OverlayState, action);
      expect(result).toEqual(initialC4OverlayState);
    });
  });
});
