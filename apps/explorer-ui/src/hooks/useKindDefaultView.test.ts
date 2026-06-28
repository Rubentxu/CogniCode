import { describe, it, expect } from "vitest";
import { useKindDefaultView } from "./useKindDefaultView";

describe("useKindDefaultView", () => {
  it("returns 'overview' for null/undefined", () => {
    expect(useKindDefaultView(null)).toBe("overview");
    expect(useKindDefaultView(undefined)).toBe("overview");
  });

  it("returns call-graph for symbol", () => {
    expect(useKindDefaultView("symbol")).toBe("call-graph");
  });

  it("returns vertical_slice for route/use_case/event", () => {
    expect(useKindDefaultView("route")).toBe("vertical_slice");
    expect(useKindDefaultView("use_case")).toBe("vertical_slice");
    expect(useKindDefaultView("event")).toBe("vertical_slice");
  });

  it("returns overview for file/scope", () => {
    expect(useKindDefaultView("file")).toBe("overview");
    expect(useKindDefaultView("scope")).toBe("overview");
  });

  it("returns evidence for decision_artifact/evidence", () => {
    expect(useKindDefaultView("decision_artifact")).toBe("evidence");
    expect(useKindDefaultView("evidence")).toBe("evidence");
  });

  it("returns quality for quality_issue/rule", () => {
    expect(useKindDefaultView("quality_issue")).toBe("quality");
    expect(useKindDefaultView("rule")).toBe("quality");
  });

  it("falls back to overview for unknown kinds", () => {
    // Cast through unknown to simulate runtime edge case
    expect(useKindDefaultView("bogus_kind" as never)).toBe("overview");
  });
});
