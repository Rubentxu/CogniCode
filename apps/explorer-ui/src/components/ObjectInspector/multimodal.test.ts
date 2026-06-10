/**
 * ObjectInspector multimodal awareness — T19 tests.
 *
 * RED gate: the helper must:
 * - Recognise the 4 multimodal `GraphNodeStyleClass` buckets
 *   (`node-decision` / `node-doc` / `node-issue` / `node-evidence`).
 * - Return 3 contextual suggestions for each.
 * - Return `null` for the 3 legacy code-only buckets.
 * - Map the legacy `InspectableObjectType::decision_artifact` /
 *   `evidence` to the right multimodal label.
 */
import { describe, expect, it } from "vitest";

import {
  MULTIMODAL_KIND_INFO,
  multimodalLabelForObjectType,
  recognizeMultimodalKind,
} from "./multimodal";

describe("recognizeMultimodalKind — T19", () => {
  it("inspector_recognizes_decision_type", () => {
    const info = recognizeMultimodalKind("node-decision");
    expect(info).not.toBeNull();
    expect(info?.badgeLabel).toBe("Decision");
  });

  it("recognises the doc style class", () => {
    const info = recognizeMultimodalKind("node-doc");
    expect(info).not.toBeNull();
    expect(info?.badgeLabel).toBe("Doc");
  });

  it("recognises the issue style class", () => {
    const info = recognizeMultimodalKind("node-issue");
    expect(info).not.toBeNull();
    expect(info?.badgeLabel).toBe("Issue");
  });

  it("recognises the evidence style class", () => {
    const info = recognizeMultimodalKind("node-evidence");
    expect(info).not.toBeNull();
    expect(info?.badgeLabel).toBe("Evidence");
  });

  it("returns null for the 3 legacy code-only buckets (regression)", () => {
    for (const sc of ["function", "module", "external"] as const) {
      expect(recognizeMultimodalKind(sc)).toBeNull();
    }
  });

  it("returns null for an unknown bucket", () => {
    expect(recognizeMultimodalKind("node-bogus")).toBeNull();
    expect(recognizeMultimodalKind(null)).toBeNull();
    expect(recognizeMultimodalKind(undefined)).toBeNull();
    expect(recognizeMultimodalKind("")).toBeNull();
  });
});

describe("MULTIMODAL_KIND_INFO — T19 suggestion surface", () => {
  it("inspector_multimodal_suggestions for decision", () => {
    const info = MULTIMODAL_KIND_INFO["node-decision"];
    expect(info.suggestions.length).toBeGreaterThanOrEqual(3);
    // Every suggestion has a stable id, label, and question.
    for (const s of info.suggestions) {
      expect(s.id).toMatch(/^dec-/);
      expect(s.label.length).toBeGreaterThan(0);
      expect(s.question.length).toBeGreaterThan(0);
    }
  });

  it("doc has 3+ contextual suggestions", () => {
    const info = MULTIMODAL_KIND_INFO["node-doc"];
    expect(info.suggestions.length).toBeGreaterThanOrEqual(3);
    for (const s of info.suggestions) {
      expect(s.id).toMatch(/^doc-/);
    }
  });

  it("issue has 3+ contextual suggestions", () => {
    const info = MULTIMODAL_KIND_INFO["node-issue"];
    expect(info.suggestions.length).toBeGreaterThanOrEqual(3);
    for (const s of info.suggestions) {
      expect(s.id).toMatch(/^iss-/);
    }
  });

  it("evidence has 3+ contextual suggestions", () => {
    const info = MULTIMODAL_KIND_INFO["node-evidence"];
    expect(info.suggestions.length).toBeGreaterThanOrEqual(3);
    for (const s of info.suggestions) {
      expect(s.id).toMatch(/^ev-/);
    }
  });
});

describe("multimodalLabelForObjectType — T19 backward compat", () => {
  it("maps decision_artifact to 'Decision'", () => {
    expect(multimodalLabelForObjectType("decision_artifact")).toBe("Decision");
  });

  it("maps evidence to 'Evidence'", () => {
    expect(multimodalLabelForObjectType("evidence")).toBe("Evidence");
  });

  it("returns null for the 7 code-only types (regression)", () => {
    for (const t of [
      "workspace",
      "scope",
      "symbol",
      "file",
      "module",
      "quality_issue",
      "rule",
    ] as const) {
      expect(multimodalLabelForObjectType(t)).toBeNull();
    }
  });
});
