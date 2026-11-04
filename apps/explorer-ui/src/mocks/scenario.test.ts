import { describe, expect, it } from "vitest";

import { activeScenarios, hasScenario, SCENARIO_PREFIX } from "./scenario";

describe("scenario flags", () => {
  it("exposes the expected scenario prefix", () => {
    expect(SCENARIO_PREFIX).toBe("__test__");
  });

  it("detects active scenarios from query string", () => {
    const search = "?__test__error=true&__test__truncated=1&__test__other=false&irrelevant=123";
    expect(activeScenarios(search)).toEqual(["error", "truncated"]);
    expect(hasScenario("error", search)).toBe(true);
    expect(hasScenario("truncated", search)).toBe(true);
    expect(hasScenario("other", search)).toBe(false);
    expect(hasScenario("missing", search)).toBe(false);
  });

  it("accepts empty affirmative values like ?__test__error", () => {
    const search = "?__test__error";
    expect(hasScenario("error", search)).toBe(true);
  });

  it("ignores non-affirmative values", () => {
    const search = "?__test__error=0&__test__truncated=no";
    expect(hasScenario("error", search)).toBe(false);
    expect(hasScenario("truncated", search)).toBe(false);
    expect(activeScenarios(search)).toEqual([]);
  });

  it("deduplicates repeated flags while preserving order", () => {
    const search = "?__test__b=true&__test__a=1&__test__b=true";
    expect(activeScenarios(search)).toEqual(["b", "a"]);
  });

  it("returns false gracefully when outside browser window and search is empty", () => {
    expect(hasScenario("error", "")).toBe(false);
    expect(activeScenarios("")).toEqual([]);
  });
});
