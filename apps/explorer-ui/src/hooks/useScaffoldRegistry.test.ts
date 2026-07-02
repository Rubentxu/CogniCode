/**
 * Tests for `useScaffoldRegistry` hook.
 *
 * Validates:
 * - All scaffolds parse successfully from the YAML fixture
 * - `filterByObjectType` returns the correct subset sorted by id
 * - `getScaffoldById` returns the correct scaffold
 * - `getScaffoldById` returns `undefined` for an unknown id
 * - `useScaffoldRegistry` returns empty array for null input
 */
import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";

import {
  filterByObjectType,
  getScaffoldById,
  useScaffoldRegistry,
} from "./useScaffoldRegistry";
import type { Scaffold } from "../api/scaffoldSchema";

describe("useScaffoldRegistry", () => {
  // -------------------------------------------------------------------------
  // filterByObjectType
  // -------------------------------------------------------------------------

  describe("filterByObjectType", () => {
    it("returns scaffolds for 'symbol' type", () => {
      const results = filterByObjectType("symbol");
      expect(results.length).toBeGreaterThan(0);
      expect(results.every((s) => s.object_type === "symbol")).toBe(true);
    });

    it("returns scaffolds for 'file' type", () => {
      const results = filterByObjectType("file");
      expect(results.length).toBeGreaterThan(0);
      expect(results.every((s) => s.object_type === "file")).toBe(true);
    });

    it("returns scaffolds for 'scope' type", () => {
      const results = filterByObjectType("scope");
      expect(results.length).toBeGreaterThan(0);
      expect(results.every((s) => s.object_type === "scope")).toBe(true);
    });

    it("returns scaffolds for 'investigation' type", () => {
      const results = filterByObjectType("investigation");
      expect(results.length).toBeGreaterThan(0);
      expect(results.every((s) => s.object_type === "investigation")).toBe(true);
    });

    it("returns empty array for unknown object type", () => {
      const results = filterByObjectType("unknown_type");
      expect(results).toEqual([]);
    });

    it("returns scaffolds sorted alphabetically by id", () => {
      const results = filterByObjectType("symbol");
      const ids = results.map((s) => s.id);
      const sorted = [...ids].sort();
      expect(ids).toEqual(sorted);
    });

    it("each scaffold has required fields populated", () => {
      const results = filterByObjectType("symbol");
      for (const scaffold of results) {
        expect(scaffold.id).toBeTruthy();
        expect(scaffold.object_type).toBe("symbol");
        expect(scaffold.intent).toBeTruthy();
        expect(scaffold.label).toBeTruthy();
        expect(scaffold.description).toBeTruthy();
        expect(scaffold.query_template).toBeTruthy();
        expect(scaffold.view_kind).toBeTruthy();
        expect(scaffold.renderer_kind).toBeTruthy();
        // Phase 1: applies_when is always null
        expect(scaffold.applies_when).toBeNull();
        // Phase 1: produces_relation_candidates is always false
        expect(scaffold.produces_relation_candidates).toBe(false);
        // query_template must contain {{object_id}} placeholder
        expect(scaffold.query_template).toContain("{{object_id}}");
      }
    });
  });

  // -------------------------------------------------------------------------
  // getScaffoldById
  // -------------------------------------------------------------------------

  describe("getScaffoldById", () => {
    it("returns the correct scaffold for 'callers_and_callees'", () => {
      const scaffold = getScaffoldById("callers_and_callees");
      expect(scaffold).toBeDefined();
      expect(scaffold!.object_type).toBe("symbol");
      expect(scaffold!.label).toBe("Callers & Callees");
      expect(scaffold!.view_kind).toBe("call_graph");
      expect(scaffold!.renderer_kind).toBe("graph");
    });

    it("returns the correct scaffold for 'file_symbols'", () => {
      const scaffold = getScaffoldById("file_symbols");
      expect(scaffold).toBeDefined();
      expect(scaffold!.object_type).toBe("file");
      expect(scaffold!.label).toBe("Symbols in File");
    });

    it("returns the correct scaffold for 'scope_symbols'", () => {
      const scaffold = getScaffoldById("scope_symbols");
      expect(scaffold).toBeDefined();
      expect(scaffold!.object_type).toBe("scope");
      expect(scaffold!.label).toBe("Scope Symbols");
    });

    it("returns undefined for unknown id", () => {
      const scaffold = getScaffoldById("nonexistent.scaffold.id");
      expect(scaffold).toBeUndefined();
    });
  });

  // -------------------------------------------------------------------------
  // useScaffoldRegistry hook
  // -------------------------------------------------------------------------

  describe("useScaffoldRegistry", () => {
    it("returns scaffolds for 'symbol' type", () => {
      const { result } = renderHook(() => useScaffoldRegistry("symbol"));
      expect(result.current.length).toBeGreaterThan(0);
      expect(result.current.every((s: Scaffold) => s.object_type === "symbol")).toBe(
        true,
      );
    });

    it("returns empty array for null input", () => {
      const { result } = renderHook(() => useScaffoldRegistry(null));
      expect(result.current).toEqual([]);
    });

    it("returns empty array for unknown type", () => {
      const { result } = renderHook(() =>
        useScaffoldRegistry("nonexistent_type"),
      );
      expect(result.current).toEqual([]);
    });
  });
});
