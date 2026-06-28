/**
 * `kindDefaultView` tests — covers all InspectableObjectType variants.
 */
import { describe, it, expect } from "vitest";
import { kindDefaultView } from "./kindDefaultView";

describe("kindDefaultView", () => {
  it("returns overview for null/undefined", () => {
    expect(kindDefaultView(null)).toBe("overview");
    expect(kindDefaultView(undefined)).toBe("overview");
  });

  it("maps symbol → call-graph", () => {
    expect(kindDefaultView("symbol")).toBe("call-graph");
  });

  it("maps route|use_case|event → vertical_slice", () => {
    expect(kindDefaultView("route")).toBe("vertical_slice");
    expect(kindDefaultView("use_case")).toBe("vertical_slice");
    expect(kindDefaultView("event")).toBe("vertical_slice");
  });

  it("maps file|scope → overview", () => {
    expect(kindDefaultView("file")).toBe("overview");
    expect(kindDefaultView("scope")).toBe("overview");
  });

  it("maps decision_artifact|evidence → evidence", () => {
    expect(kindDefaultView("decision_artifact")).toBe("evidence");
    expect(kindDefaultView("evidence")).toBe("evidence");
  });

  it("maps workspace|module → overview", () => {
    expect(kindDefaultView("workspace")).toBe("overview");
    expect(kindDefaultView("module")).toBe("overview");
  });

  it("maps quality_issue|rule → quality", () => {
    expect(kindDefaultView("quality_issue")).toBe("quality");
    expect(kindDefaultView("rule")).toBe("quality");
  });

  it("falls back to overview for unknown kinds", () => {
    expect(kindDefaultView("bogus_kind" as never)).toBe("overview");
  });
});
