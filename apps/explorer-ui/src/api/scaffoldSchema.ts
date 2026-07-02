/**
 * Zod schema for the MoldQL scaffold registry.
 *
 * Mirrors the Rust `Scaffold` struct defined in
 * `crates/cognicode-explorer/src/scaffold.rs`. Each scaffold is a
 * declarative recipe loaded from `moldql-scaffolds.yaml` and re-validated
 * on the TypeScript side with Zod for type safety at the UI boundary.
 */
import { z } from "zod";

import {
  inspectableObjectTypeSchema,
  viewKindSchema,
  rendererKindSchema,
} from "./schemas";

// ---------------------------------------------------------------------------
// Scaffold schema
// ---------------------------------------------------------------------------

/**
 * A single scaffold definition parsed from `moldql-scaffolds.yaml`.
 *
 * Corresponds to the Rust `Scaffold` struct in `scaffold.rs`.
 */
export const scaffoldSchema = z.object({
  /** Unique identifier, e.g. `"callers_and_callees"`. */
  id: z.string(),

  /** Which inspectable object type this scaffold applies to. */
  object_type: inspectableObjectTypeSchema,

  /** One-line semantic intent (imperative mood, e.g. "Find callers"). */
  intent: z.string(),

  /** Short display label for UI pickers. */
  label: z.string(),

  /** Longer explanation shown in hover / help text. */
  description: z.string(),

  /**
   * MoldQL query string with `{{object_id}}` placeholder substituted
   * at runtime with the focused object's id.
   */
  query_template: z.string(),

  /** Recommended `ViewKind` variant name (snake_case string). */
  view_kind: viewKindSchema,

  /** Recommended `RendererKind` variant name (snake_case string). */
  renderer_kind: rendererKindSchema,

  /**
   * Conditional eligibility predicate — `null` in Phase 1
   * (future: conditional eligibility expressions).
   */
  applies_when: z.string().nullable().default(null),

  /**
   * Whether this scaffold produces relation-candidate edges —
   * always `false` in Phase 1.
   */
  produces_relation_candidates: z.boolean().default(false),
});

export type Scaffold = z.infer<typeof scaffoldSchema>;

// ---------------------------------------------------------------------------
// Scaffold registry
// ---------------------------------------------------------------------------

/** The top-level YAML structure is a map with a `scaffolds` key. */
export const scaffoldRegistrySchema = z.object({
  scaffolds: z.array(scaffoldSchema),
});

export type ScaffoldRegistry = z.infer<typeof scaffoldRegistrySchema>;
