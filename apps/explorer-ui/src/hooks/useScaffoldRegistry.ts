/**
 * `useScaffoldRegistry` — React hook for accessing the MoldQL scaffold registry.
 *
 * Scaffolds are declarative view recipes loaded from `moldql-scaffolds.yaml`
 * at build time via Vite's YAML plugin. The hook provides typed access to
 * scaffolds filtered by `InspectableObjectType`, mirroring the Rust
 * `ScaffoldRegistry::list_for` and `get` methods.
 *
 * @see `crates/cognicode-explorer/src/scaffold.rs` for the Rust source of truth.
 */
import { useMemo } from "react";

import moldqlScaffolds from "@scaffold-assets/moldql-scaffolds.yaml";

import {
  scaffoldRegistrySchema,
  type Scaffold,
} from "../api/scaffoldSchema";

// ---------------------------------------------------------------------------
// Module-level parse cache — validated once at import time
// ---------------------------------------------------------------------------

/** Scaffolds extracted and validated from the YAML file. */
const ALL_SCAFFOLDS: Scaffold[] = (() => {
  const parsed = scaffoldRegistrySchema.parse(moldqlScaffolds);
  return parsed.scaffolds;
})();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Return all scaffolds whose `object_type` matches the given type.
 *
 * Results are sorted alphabetically by `id`.
 *
 * @param objectType - The `InspectableObjectType` to filter by
 */
export function filterByObjectType(objectType: string): Scaffold[] {
  return ALL_SCAFFOLDS.filter((s) => s.object_type === objectType).sort(
    (a, b) => a.id.localeCompare(b.id),
  );
}

/**
 * Return the scaffold with the given `id`, or `undefined` if not found.
 *
 * @param id - The scaffold identifier (e.g. `"callers_and_callees"`)
 */
export function getScaffoldById(id: string): Scaffold | undefined {
  return ALL_SCAFFOLDS.find((s) => s.id === id);
}

/**
 * React hook returning all scaffolds for a given `InspectableObjectType`.
 *
 * @param objectType - The `InspectableObjectType` to filter by
 *                     (pass `null` to get an empty array)
 */
export function useScaffoldRegistry(objectType: string | null): Scaffold[] {
  return useMemo(() => {
    if (!objectType) return [];
    return filterByObjectType(objectType);
  }, [objectType]);
}
