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

  it("maps route → vertical_slice", () => {
    expect(kindDefaultView("route")).toBe("vertical_slice");
  });

  it("maps file → overview, scope → seam_map", () => {
    expect(kindDefaultView("file")).toBe("overview");
    expect(kindDefaultView("scope")).toBe("seam_map");
  });

  it("maps doc → doc_code_alignment", () => {
    expect(kindDefaultView("doc")).toBe("doc_code_alignment");
  });

  it("maps decision_artifact → doc_code_alignment", () => {
    expect(kindDefaultView("decision_artifact")).toBe("doc_code_alignment");
  });

  it("maps evidence → evidence", () => {
    expect(kindDefaultView("evidence")).toBe("evidence");
  });

  it("maps workspace → overview, module → seam_map", () => {
    expect(kindDefaultView("workspace")).toBe("overview");
    expect(kindDefaultView("module")).toBe("seam_map");
  });

  it("maps quality_issue|rule → quality", () => {
    expect(kindDefaultView("quality_issue")).toBe("quality");
    expect(kindDefaultView("rule")).toBe("quality");
  });

  it("maps investigation → overview", () => {
    expect(kindDefaultView("investigation")).toBe("overview");
  });

  it("falls back to overview for unknown kinds", () => {
    expect(kindDefaultView("bogus_kind" as never)).toBe("overview");
  });
});
