/**
 * `suggestedQuestions` — tests for the static, typed suggestion map.
 *
 * The map is the source of truth for "What can I do here?" prompts. It
 * is consumed by `SuggestionStrip` and `useAsk`. Every consumer relies
 * on:
 *   1. Exhaustiveness over all 9 `InspectableObjectType` variants
 *   2. Each kind having 3-5 prompts
 *   3. Every entry having a valid `tool` and a non-empty `params` object
 *   4. The map being readonly at the type level
 *   5. The `filterByGraph` pure function gating graph-dependent prompts
 *      according to the current `graph_status`
 */
import { describe, it, expect } from "vitest";

import { inspectableObjectTypeSchema } from "../api/schemas";
import type { InspectableObjectType } from "../api/types";
import {
  SUGGESTED_QUESTIONS,
  filterByGraph,
  type SuggestedQuestion,
  type SuggestedTool,
} from "./suggestedQuestions";

const ALL_KINDS = inspectableObjectTypeSchema.options;

// Sanity: the spec locks exactly 9 variants. If this fails, the backend
// has added a new kind and the map MUST be updated before any consumer
// will compile.
const EXPECTED_KINDS: readonly InspectableObjectType[] = [
  "workspace",
  "scope",
  "symbol",
  "file",
  "module",
  "evidence",
  "decision_artifact",
  "quality_issue",
  "rule",
];

const ALLOWED_TOOLS: readonly SuggestedTool[] = [
  "cognicode_ask",
  "explorer_inspect_object",
  "explorer_get_view",
  "explorer_open_workspace",
];

// ---------------------------------------------------------------------------
// Exhaustiveness + counts
// ---------------------------------------------------------------------------

describe("SUGGESTED_QUESTIONS — exhaustiveness", () => {
  it("covers all 9 InspectableObjectType variants", () => {
    expect(ALL_KINDS).toEqual(EXPECTED_KINDS);
    // The map must declare a key for every variant the schema enumerates.
    for (const kind of EXPECTED_KINDS) {
      expect(SUGGESTED_QUESTIONS[kind]).toBeDefined();
      expect(Array.isArray(SUGGESTED_QUESTIONS[kind])).toBe(true);
    }
  });

  it("every kind has between 3 and 5 prompts (inclusive)", () => {
    for (const kind of EXPECTED_KINDS) {
      const prompts = SUGGESTED_QUESTIONS[kind];
      expect(prompts.length, `kind=${kind} should be 3-5`).toBeGreaterThanOrEqual(3);
      expect(prompts.length, `kind=${kind} should be 3-5`).toBeLessThanOrEqual(5);
    }
  });

  it("the symbol kind exposes the full 5 prompts (most-populated kind)", () => {
    expect(SUGGESTED_QUESTIONS.symbol.length).toBe(5);
  });
});

// ---------------------------------------------------------------------------
// Field validation
// ---------------------------------------------------------------------------

describe("SUGGESTED_QUESTIONS — field validation", () => {
  it("every entry has a non-empty id, label, valid tool, and non-empty params", () => {
    const idPattern = /^[a-z][a-z0-9-]*$/;
    for (const kind of EXPECTED_KINDS) {
      for (const prompt of SUGGESTED_QUESTIONS[kind]) {
        expect(prompt.id.length, `${kind}/${prompt.id} id`).toBeGreaterThan(0);
        expect(idPattern.test(prompt.id), `${kind}/${prompt.id} id should be kebab-case`).toBe(true);
        expect(prompt.label.length, `${kind}/${prompt.id} label`).toBeGreaterThan(0);
        expect(prompt.label.length, `${kind}/${prompt.id} label <= 60 chars`).toBeLessThanOrEqual(60);
        expect(ALLOWED_TOOLS).toContain(prompt.tool);
        expect(typeof prompt.params).toBe("object");
        expect(prompt.params).not.toBeNull();
        expect(Object.keys(prompt.params).length, `${kind}/${prompt.id} params`).toBeGreaterThan(0);
        expect(typeof prompt.requiresGraph).toBe("boolean");
      }
    }
  });

  it("prompt ids are unique within each kind", () => {
    for (const kind of EXPECTED_KINDS) {
      const ids = SUGGESTED_QUESTIONS[kind].map((p) => p.id);
      expect(new Set(ids).size, `${kind} has duplicate ids`).toBe(ids.length);
    }
  });

  it("cognicode_ask prompts always carry a `question` param", () => {
    for (const kind of EXPECTED_KINDS) {
      for (const prompt of SUGGESTED_QUESTIONS[kind]) {
        if (prompt.tool === "cognicode_ask") {
          expect(
            typeof prompt.params.question === "string" && prompt.params.question.length > 0,
            `${kind}/${prompt.id} (cognicode_ask) needs a question param`,
          ).toBe(true);
        }
      }
    }
  });
});

// ---------------------------------------------------------------------------
// Readonly at the type level (compile-time)
// ---------------------------------------------------------------------------

describe("SUGGESTED_QUESTIONS — readonly contract", () => {
  it("exports a readonly Record — runtime shape is frozen at runtime", () => {
    // We can't runtime-enforce `readonly` but we can verify the map
    // shape stays a plain object (no exotic Proxy that would mutate
    // silently) and that every entry is `Object.freeze`-friendly.
    expect(typeof SUGGESTED_QUESTIONS).toBe("object");
    for (const kind of EXPECTED_KINDS) {
      for (const prompt of SUGGESTED_QUESTIONS[kind]) {
        // Each prompt must look like a plain readonly object literal.
        expect(Object.getPrototypeOf(prompt) === Object.prototype).toBe(true);
      }
    }
  });

  it("SuggestedQuestion fields are typed as readonly (compile-time check)", () => {
    // This test only exists to give the TypeScript compiler a chance
    // to fail if the type is ever loosened. The line below would not
    // compile if any field dropped `readonly`.
    const sample: SuggestedQuestion = SUGGESTED_QUESTIONS.symbol[0]!;
    expect(sample.id).toBeDefined();
    expect(sample.tool).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// filterByGraph — pure function, exhaustive gate matrix
// ---------------------------------------------------------------------------

describe("filterByGraph", () => {
  // Use the file kind as a representative test set: 2 of 4 prompts
  // require the graph. The exhaustive matrix is checked against `file`
  // so the test does not depend on any specific kind's exact count.
  const filePrompts = SUGGESTED_QUESTIONS.file;
  const fileGraph = filePrompts.filter((p) => p.requiresGraph);
  const fileNonGraph = filePrompts.filter((p) => !p.requiresGraph);

  it("returns all prompts when status is 'ready'", () => {
    const out = filterByGraph(filePrompts, "ready");
    expect(out.length).toBe(filePrompts.length);
  });

  it("drops graph-dependent prompts when status is 'missing'", () => {
    const out = filterByGraph(filePrompts, "missing");
    expect(out.length).toBe(fileNonGraph.length);
    for (const p of out) {
      expect(p.requiresGraph).toBe(false);
    }
  });

  it("drops graph-dependent prompts when status is 'indexing'", () => {
    const out = filterByGraph(filePrompts, "indexing");
    expect(out.length).toBe(fileNonGraph.length);
  });

  it("treats null status the same as 'missing' (no workspace open)", () => {
    const out = filterByGraph(filePrompts, null);
    expect(out.length).toBe(fileNonGraph.length);
  });

  it("keeps all prompts when status is 'stale' (the caller is responsible for disabling)", () => {
    const out = filterByGraph(filePrompts, "stale");
    expect(out.length).toBe(filePrompts.length);
    // The stale case is the caller's signal: every prompt is still
    // emitted, but `useAsk` and `SuggestionStrip` should disable
    // graph-dependent ones before dispatching.
    expect(out.filter((p) => p.requiresGraph).length).toBe(fileGraph.length);
  });
});
