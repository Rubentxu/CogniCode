/**
 * Zod schemas for the affordance matrix API.
 *
 * The backend returns a raw array of affordance objects at GET /affordances/:object_type.
 */
import { z } from "zod";

export const affordanceSchema = z.object({
  object_type: z.string(),
  label: z.string(),
  description: z.string(),
  view_kind: z.string(),
  scaffold_id: z.string().nullable(),
  priority: z.number().int().nonnegative(),
});
export type Affordance = z.infer<typeof affordanceSchema>;

/** The backend returns a raw array of affordances. */
export type AffordancesResponse = Affordance[];
